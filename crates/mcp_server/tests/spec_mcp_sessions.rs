//! #250 stateful-session integration tests (spectra: mcp-stateful-sessions).
//!
//! Real TCP round-trips against a spawned server: initialize mints an
//! Mcp-Session-Id, requests route by it, unknown/expired ids are 404,
//! header-less clients keep the legacy shared behavior, DELETE terminates,
//! and an SSE stream receives notifications/tools/list_changed.

use agent_contract::ToolExecutor;
use core_model::{MediaManifest, Timeline};
use mcp_server::{McpConfig, McpServer, McpServerHandle};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn spawn_server() -> McpServerHandle {
    let config = McpConfig {
        port: 0,
        ..Default::default()
    };
    let exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
    McpServer::new(config, exec).spawn().expect("spawn")
}

fn raw_round_trip(port: u16, request: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    s.flush().unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

fn post(port: u16, extra_headers: &str, body: &str) -> String {
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\n\r\n{}",
        extra_headers,
        body.len(),
        body
    );
    raw_round_trip(port, &req)
}

fn header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    let head = response.split("\r\n\r\n").next()?;
    head.split("\r\n").skip(1).find_map(|line| {
        let (n, v) = line.split_once(':')?;
        n.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;

#[test]
fn mcp_250_initialize_mints_a_session_id() {
    let h = spawn_server();
    let resp = post(h.port(), "", INIT);
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    let sid = header_value(&resp, "Mcp-Session-Id");
    assert!(
        sid.is_some_and(|v| !v.is_empty()),
        "initialize must return Mcp-Session-Id: {resp}"
    );
    h.stop();
}

#[test]
fn mcp_250_session_routes_and_unknown_is_404() {
    let h = spawn_server();
    let init = post(h.port(), "", INIT);
    let sid = header_value(&init, "Mcp-Session-Id").expect("sid").to_string();

    let ok = post(
        h.port(),
        &format!("Mcp-Session-Id: {sid}\r\n"),
        TOOLS_LIST,
    );
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("\"tools\""), "{ok}");

    let bad = post(h.port(), "Mcp-Session-Id: nonexistent-id\r\n", TOOLS_LIST);
    assert!(
        bad.starts_with("HTTP/1.1 404"),
        "unknown session must be 404 per MCP streamable HTTP: {bad}"
    );
    assert!(bad.to_ascii_lowercase().contains("session"), "{bad}");
    h.stop();
}

#[test]
fn mcp_250_headerless_client_keeps_legacy_behavior() {
    let h = spawn_server();
    let resp = post(h.port(), "", TOOLS_LIST);
    assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
    assert!(resp.contains("\"tools\""), "{resp}");
    h.stop();
}

#[test]
fn mcp_250_delete_terminates_the_session() {
    let h = spawn_server();
    let init = post(h.port(), "", INIT);
    let sid = header_value(&init, "Mcp-Session-Id").expect("sid").to_string();

    let del = raw_round_trip(
        h.port(),
        &format!("DELETE /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nMcp-Session-Id: {sid}\r\n\r\n"),
    );
    assert!(del.starts_with("HTTP/1.1 200"), "{del}");

    let after = post(h.port(), &format!("Mcp-Session-Id: {sid}\r\n"), TOOLS_LIST);
    assert!(after.starts_with("HTTP/1.1 404"), "{after}");
    h.stop();
}

#[test]
fn mcp_250_body_split_across_packets_is_read_fully() {
    // Content-Length framing: send head and body in two writes.
    let h = spawn_server();
    let mut s = TcpStream::connect(("127.0.0.1", h.port())).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        TOOLS_LIST.len()
    );
    s.write_all(head.as_bytes()).unwrap();
    s.flush().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    s.write_all(TOOLS_LIST.as_bytes()).unwrap();
    s.flush().unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert!(out.contains("\"tools\""), "{out}");
    h.stop();
}

#[test]
fn mcp_250_sse_stream_receives_tools_list_changed() {
    let h = spawn_server();
    let init = post(h.port(), "", INIT);
    let sid = header_value(&init, "Mcp-Session-Id").expect("sid").to_string();

    let mut s = TcpStream::connect(("127.0.0.1", h.port())).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/event-stream\r\nMcp-Session-Id: {sid}\r\n\r\n"
    );
    s.write_all(req.as_bytes()).unwrap();
    s.flush().unwrap();

    let mut reader = BufReader::new(s);
    // Read the response head first.
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("head line");
        if line == "\r\n" {
            break;
        }
        head.push_str(&line);
    }
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(
        head.to_ascii_lowercase().contains("text/event-stream"),
        "{head}"
    );

    // Trigger a broadcast and expect the event frame on the stream.
    std::thread::sleep(Duration::from_millis(100));
    h.notify_tools_changed();

    let mut got = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                got.push_str(&line);
                if got.contains("notifications/tools/list_changed") && line == "\n" {
                    break;
                }
            }
        }
    }
    assert!(
        got.contains("notifications/tools/list_changed"),
        "SSE stream must carry the list_changed notification: {got}"
    );
    h.stop();
}
