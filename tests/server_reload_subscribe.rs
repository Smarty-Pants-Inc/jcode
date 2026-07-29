//! Regression test for `jcode server reload` protocol ordering.
//!
//! The server rejects stateful requests from a client that has not yet
//! subscribed with a working_dir. `jcode server reload` used to connect and
//! send `Reload` immediately, so it always failed with:
//!
//!   Client must Subscribe with a working_dir before sending stateful requests
//!
//! This test drives the real `jcode server reload` binary against a stub
//! server and asserts the *first* frame it sends is `Subscribe`. Reverting the
//! fix makes this fail, because the first frame becomes `Reload`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

fn jcode_binary() -> PathBuf {
    // The integration test binary lives in target/<profile>/deps/.
    let mut path = std::env::current_exe().expect("current_exe");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("jcode")
}

#[test]
fn server_reload_subscribes_before_sending_reload() {
    let binary = jcode_binary();
    if !binary.exists() {
        eprintln!("skipping: {} not built", binary.display());
        return;
    }

    let dir = std::env::temp_dir().join(format!("jcode-reload-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let socket = dir.join("jcode.sock");
    let _ = std::fs::remove_file(&socket);

    let listener = UnixListener::bind(&socket).expect("bind stub socket");
    let (tx, rx) = mpsc::channel::<String>();

    // Stub server: `jcode server reload` first opens a throwaway liveness-probe
    // connection and closes it without sending anything, so keep accepting
    // until a connection actually delivers a frame.
    let server = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let mut line = String::new();
            match reader.read_line(&mut line) {
                // Probe connection: closed with no data. Wait for the next one.
                Ok(0) => continue,
                Ok(_) => {}
                Err(_) => continue,
            }
            let _ = tx.send(line.clone());
            // Acknowledge the Subscribe so the client is not left hanging.
            let mut writer = stream;
            let id = serde_json::from_str::<serde_json::Value>(&line)
                .ok()
                .and_then(|v| v.get("id").and_then(|i| i.as_u64()))
                .unwrap_or(1);
            let ack = format!(
                "{}\n",
                serde_json::json!({ "type": "subscribed", "id": id })
            );
            let _ = writer.write_all(ack.as_bytes());
            let _ = writer.flush();
            return;
        }
    });

    let mut child = Command::new(&binary)
        .args(["server", "reload"])
        .env("JCODE_SOCKET", &socket)
        .spawn()
        .expect("spawn jcode server reload");

    let first_frame = rx.recv_timeout(Duration::from_secs(30));
    let _ = child.kill();
    let _ = child.wait();
    let _ = server.join();
    let _ = std::fs::remove_file(&socket);
    let _ = std::fs::remove_dir_all(&dir);

    let first_frame = first_frame.expect("client sent no frame to the server");
    let parsed: serde_json::Value =
        serde_json::from_str(&first_frame).expect("first frame is not valid JSON");
    let frame_type = parsed
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    assert_eq!(
        frame_type, "subscribe",
        "`jcode server reload` must Subscribe before sending stateful requests, \
         but its first frame was `{frame_type}`; the server rejects this with \
         \"Client must Subscribe with a working_dir before sending stateful requests\""
    );

    let working_dir = parsed
        .get("working_dir")
        .and_then(|w| w.as_str())
        .unwrap_or_default();
    assert!(
        !working_dir.is_empty() && std::path::Path::new(working_dir).is_absolute(),
        "Subscribe must carry an absolute working_dir, got {working_dir:?}"
    );
}
