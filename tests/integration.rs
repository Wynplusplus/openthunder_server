//! End-to-end test: start the real server binary, connect a raw client, join,
//! send state and receive a snapshot.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::Duration;

fn spawn_server(port: u16) -> Child {
    Command::new(env!("CARGO_BIN_EXE_openthunder_server"))
        .args([
            "--bind",
            &format!("127.0.0.1:{port}"),
            "--maps",
            "maps",
            "--map",
            "training",
        ])
        .spawn()
        .expect("spawn server")
}

#[test]
fn client_joins_and_receives_snapshots() {
    let port = 7788;
    let mut server = spawn_server(port);

    // Wait for the listener to come up.
    let mut stream = None;
    for _ in 0..50 {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let stream = stream.expect("server should accept connections");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    // Join.
    writeln!(writer, "JOIN\t2\tWyn\tF4U-4 Corsair").unwrap();
    writer.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.starts_with("WELCOME"),
        "expected WELCOME, got {line:?}"
    );
    assert!(line.contains("Training Islands"), "map name in WELCOME");

    // The server ships its planes and pilot crew config right after WELCOME.
    let mut saw_planes = false;
    let mut saw_crew = false;
    for _ in 0..3 {
        line.clear();
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

    let mut saw_snapshot = false;
    for _ in 0..40 {
        line.clear();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        if line.starts_with("SNAPSHOT") && line.contains("F4U-4 Corsair") {
            saw_snapshot = true;
            break;
        }
    }
    assert!(saw_snapshot, "expected a snapshot containing our aircraft");

    let _ = server.kill();
    let _ = server.wait();
}
