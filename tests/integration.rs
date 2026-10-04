//! End-to-end tests: start the real server binary, connect raw clients, join,
//! send state and receive snapshots (and, for TDM, team/kill/score messages).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::Duration;

/// Kills the server process when dropped, so a failing test cannot leak it.
struct ServerGuard(Child);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_server(port: u16, map: &str) -> ServerGuard {
    let child = Command::new(env!("CARGO_BIN_EXE_openthunder_server"))
        .args([
            "--bind",
            &format!("127.0.0.1:{port}"),
            "--maps",
            "maps",
            "--map",
            map,
        ])
        .spawn()
        .expect("spawn server");
    ServerGuard(child)
}

/// Wait for the listener to accept connections.
fn wait_for_server(port: u16) -> TcpStream {
    for _ in 0..50 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            return stream;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server should accept connections");
}

/// Connect, join and return `(writer, reader, welcome_line)`.
fn join(port: u16, name: &str) -> (TcpStream, BufReader<TcpStream>, String) {
    let stream = wait_for_server(port);
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    writeln!(writer, "JOIN\t3\t{name}\tF4U-4 Corsair").unwrap();
    writer.flush().unwrap();
    let mut welcome = String::new();
    reader.read_line(&mut welcome).unwrap();
    (writer, reader, welcome)
}

/// Read lines until `pred` matches, or `limit` lines have been read.
fn read_until(reader: &mut impl BufRead, limit: usize, pred: impl Fn(&str) -> bool) -> bool {
    for _ in 0..limit {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            return false;
        }
        if pred(&line) {
            return true;
        }
    }
    false
}

#[test]
fn client_joins_and_receives_snapshots() {
    let port = 7788;
    let _server = spawn_server(port, "training");

    let (mut writer, mut reader, welcome) = join(port, "Wyn");
    assert!(
        welcome.starts_with("WELCOME"),
        "expected WELCOME, got {welcome:?}"
    );
    assert!(welcome.contains("Training Islands"), "map name in WELCOME");

    // The server ships its planes and pilot crew config right after WELCOME.
    let mut saw_planes = false;
    let mut saw_crew = false;
    for _ in 0..3 {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        saw_planes |= line.starts_with("PLANES");
        if line.starts_with("CREW") {
            saw_crew = true;
            assert!(
                line.contains("g_tolerance"),
                "crew config should carry g_tolerance, got {line:?}"
            );
        }
    }
    assert!(saw_planes, "expected a PLANES message");
    assert!(saw_crew, "expected a CREW message");

    // Send state and wait for a snapshot that contains us.
    writeln!(
        writer,
        "STATE\t0.000\t1000.000\t0.000\t0.000\t0.000\t0.000\t1.000\t0.000\t0.000\t-150.000"
    )
    .unwrap();
    writer.flush().unwrap();
    assert!(
        read_until(&mut reader, 40, |line| line.starts_with("SNAPSHOT")
            && line.contains("F4U-4 Corsair")),
        "expected a snapshot containing our aircraft"
    );
}

#[test]
fn team_deathmatch_assigns_teams_and_scores_kills() {
    let port = 7791;
    let _server = spawn_server(port, "pacific_islands");

    let (mut first, _first_reader, welcome_first) = join(port, "Red");
    let (mut second, mut second_reader, welcome_second) = join(port, "Blue");

    assert!(
        welcome_first.contains("Pacific Islands"),
        "map name in WELCOME: {welcome_first:?}"
    );
    // The two players are split across the two teams.
    assert!(
        welcome_first.trim_end().ends_with("\t0"),
        "first player should be on team 0: {welcome_first:?}"
    );
    assert!(
        welcome_second.trim_end().ends_with("\t1"),
        "second player should be on team 1: {welcome_second:?}"
    );

    // Wait until the second client is receiving snapshots: its writer is now
    // registered, so the relayed HIT below cannot be missed.
    assert!(
        read_until(&mut second_reader, 60, |line| line.starts_with("SNAPSHOT")),
        "second client should receive snapshots"
    );

    // First player hits the second (id 2), who then reports death.
    writeln!(first, "HIT\t2\t0\t10.0").unwrap();
    first.flush().unwrap();
    assert!(
        read_until(&mut second_reader, 60, |line| line.starts_with("HIT\t2")),
        "expected the relayed HIT"
    );

    writeln!(second, "DEATH").unwrap();
    second.flush().unwrap();

    // Second player should receive a KILL and a MATCH with a 1-0 score.
    let mut saw_kill = false;
    let mut saw_score = false;
    for _ in 0..80 {
        let mut line = String::new();
        if second_reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        saw_kill |= line.starts_with("KILL\t1\t2");
        if line.starts_with("MATCH") && line.contains("\t1\t0") {
            saw_score = true;
            break;
        }
    }
    assert!(saw_kill, "expected a KILL message crediting the attacker");
    assert!(saw_score, "expected a MATCH message with team 0 on 1 kill");
}
