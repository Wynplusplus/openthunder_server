//! The dedicated server: accepts clients over TCP, relays aircraft state, and
//! runs the gamemode tick.
//!
//! Movement is **client-authoritative** for now (each client simulates its own
//! aircraft and streams its state); the server keeps the authoritative roster
//! and broadcasts snapshots. That is enough for a co-op/free-flight prototype
//! and leaves room to move simulation server-side later.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::gamemode::{self, GameMode, PlayerView, TickContext};
use crate::map::MapConfig;
use crate::protocol::{ClientMessage, PlayerSnapshot, ServerMessage};

/// Snapshot rate.
const TICK_HZ: f32 = 20.0;

#[derive(Clone, Debug)]
struct Player {
    name: String,
    plane: String,
    position: [f32; 3],
    rotation: [f32; 4],
    velocity: [f32; 3],
}

impl Default for Player {
    fn default() -> Self {
        Self {
            name: String::new(),
            plane: String::new(),
            position: [0.0, 1000.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0, 0.0, 0.0],
        }
    }
}

struct Shared {
    players: Mutex<HashMap<u64, Player>>,
    /// A write handle per connected client, used by the tick thread.
    writers: Mutex<HashMap<u64, TcpStream>>,
    next_id: AtomicU64,
}

impl Shared {
    fn snapshot(&self) -> Vec<PlayerSnapshot> {
        self.players
            .lock()
            .unwrap()
            .iter()
            .map(|(id, player)| PlayerSnapshot {
                id: *id,
                name: player.name.clone(),
                plane: player.plane.clone(),
                position: player.position,
                rotation: player.rotation,
                velocity: player.velocity,
            })
            .collect()
    }

    fn player_views(&self) -> Vec<PlayerView> {
        self.players
            .lock()
            .unwrap()
            .iter()
            .map(|(id, player)| PlayerView {
                id: *id,
                name: player.name.clone(),
                plane: player.plane.clone(),
                position: player.position,
            })
            .collect()
    }

    fn broadcast(&self, message: &ServerMessage) {
        let line = format!("{}\n", message.to_line());
        let mut writers = self.writers.lock().unwrap();
        writers.retain(|_, stream| stream.write_all(line.as_bytes()).is_ok());
    }
}

/// Run the server on `bind` with the given map until the process is killed.
pub fn run(bind: &str, map: MapConfig) -> std::io::Result<()> {
    let mut mode = gamemode::create(&map.gamemode).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "unknown gamemode '{}' (known: {})",
                map.gamemode,
                gamemode::registered_ids().join(", ")
            ),
        )
    })?;
    mode.on_start(&map);

    let max_players = map.rule_u32("max_players", 16) as usize;
    let listener = TcpListener::bind(bind)?;
    println!(
        "[server] '{}' listening on {bind}  (gamemode: {}, max players: {max_players})",
        map.name,
        mode.name(),
    );

    let shared = Arc::new(Shared {
        players: Mutex::new(HashMap::new()),
        writers: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
    });

    // Snapshot / gamemode tick thread.
    {
        let shared = Arc::clone(&shared);
        let map = map.clone();
        std::thread::spawn(move || tick_loop(shared, map, mode));
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let shared = Arc::clone(&shared);
                let map = map.clone();
                std::thread::spawn(move || {
                    if let Err(err) = handle_client(stream, shared, map, max_players) {
                        eprintln!("[server] client error: {err}");
                    }
                });
            }
            Err(err) => eprintln!("[server] accept error: {err}"),
        }
    }
    Ok(())
}

fn handle_client(
    stream: TcpStream,
    shared: Arc<Shared>,
    map: MapConfig,
    max_players: usize,
) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let peer = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "?".into());

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // The first message must be a JOIN.
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let (name, plane) = match ClientMessage::parse(&line) {
        Ok(ClientMessage::Join { name, plane }) => (name, plane),
        Ok(_) => {
            let _ = write_line(&mut writer, &ServerMessage::Error {
                reason: "expected JOIN first".into(),
            });
            return Ok(());
        }
        Err(err) => {
            let _ = write_line(&mut writer, &ServerMessage::Error {
                reason: err.0,
            });
            return Ok(());
        }
    };

    if shared.players.lock().unwrap().len() >= max_players {
        let _ = write_line(&mut writer, &ServerMessage::Error {
            reason: "server full".into(),
        });
        return Ok(());
    }

    let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
    shared.players.lock().unwrap().insert(
        id,
        Player {
            name: name.clone(),
            plane: plane.clone(),
            ..Player::default()
        },
    );
    write_line(&mut writer, &ServerMessage::Welcome {
        id,
        map: map.name.clone(),
        gamemode: map.gamemode.clone(),
    })?;
    shared
        .writers
        .lock()
        .unwrap()
        .insert(id, writer.try_clone()?);
    println!("[server] + {name} ({plane}) id={id} from {peer}");

    let result = read_loop(&mut reader, &shared, id);

    shared.players.lock().unwrap().remove(&id);
    shared.writers.lock().unwrap().remove(&id);
    shared.broadcast(&ServerMessage::PlayerLeft { id });
    println!("[server] - {name} id={id}");
    result
}

fn read_loop(reader: &mut impl BufRead, shared: &Shared, id: u64) -> std::io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(()); // disconnected
        }
        match ClientMessage::parse(&line) {
            Ok(ClientMessage::State {
                position,
                rotation,
                velocity,
            }) => {
                if let Some(player) = shared.players.lock().unwrap().get_mut(&id) {
                    player.position = position;
                    player.rotation = rotation;
                    player.velocity = velocity;
                }
            }
            Ok(ClientMessage::Leave) => return Ok(()),
            Ok(ClientMessage::Join { .. }) => {}
            Err(err) => eprintln!("[server] bad message from id={id}: {err}"),
        }
    }
}

fn tick_loop(shared: Arc<Shared>, map: MapConfig, mut mode: Box<dyn GameMode>) {
    let start = Instant::now();
    let period = Duration::from_secs_f32(1.0 / TICK_HZ);
    let mut tick: u64 = 0;
    loop {
        std::thread::sleep(period);
        tick += 1;
        let views = shared.player_views();
        let ctx = TickContext {
            map: &map,
            players: &views,
            elapsed_secs: start.elapsed().as_secs_f32(),
            tick,
        };
        if mode.on_tick(&ctx).round_over {
            println!("[server] round over (tick {tick})");
        }
        shared.broadcast(&ServerMessage::Snapshot {
            players: shared.snapshot(),
        });
    }
}

fn write_line(stream: &mut TcpStream, message: &ServerMessage) -> std::io::Result<()> {
    stream.write_all(message.to_line().as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}
