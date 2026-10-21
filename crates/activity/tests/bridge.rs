//! The browser bridge accepts only paired extensions on this machine.

use std::io::{Read, Write};
use std::net::TcpStream;

use nudgy_activity::{Bridge, now_ms};

const SECRET: &str = "s3cret-s3cret-s3cret";

fn bridge() -> (Bridge, u16) {
    let bridge = Bridge::start(&[0], SECRET.into());
    let port = bridge.port().expect("listening");
    (bridge, port)
}

/// Sends a raw HTTP request; returns (status, body).
fn http(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !headers.iter().any(|(k, _)| *k == "Host") {
        req.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
    (status, body)
}

fn heartbeat(port: u16, headers: &[(&str, &str)], body: &str) -> u16 {
    let mut all = vec![("Content-Type", "application/json")];
    all.extend_from_slice(headers);
    http(port, "POST", "/v1/heartbeat", &all, body).0
}

const CHROME_TAB: &str = r#"{"browser":"chrome","domain":"instagram.com","title":"Instagram","incognito":false,"at":0}"#;

#[test]
fn heartbeats_need_the_secret() {
    let (bridge, port) = bridge();
    assert_eq!(heartbeat(port, &[], CHROME_TAB), 401);
    assert_eq!(heartbeat(port, &[("X-Nudgy-Secret", "wrong")], CHROME_TAB), 401);
    assert!(bridge.heartbeat_for("chrome", now_ms()).is_none());

    assert_eq!(heartbeat(port, &[("X-Nudgy-Secret", SECRET)], CHROME_TAB), 204);
    let hb = bridge.heartbeat_for("chrome", now_ms()).unwrap();
    assert_eq!(hb.domain, "instagram.com");
    assert!(bridge.heartbeat_for("edge", now_ms()).is_none());
}

#[test]
fn heartbeats_go_stale() {
    let (bridge, port) = bridge();
    assert_eq!(heartbeat(port, &[("X-Nudgy-Secret", SECRET)], CHROME_TAB), 204);
    assert!(bridge.heartbeat_for("chrome", now_ms() + 60_000).is_some());
    assert!(bridge.heartbeat_for("chrome", now_ms() + 120_000).is_none());
}

#[test]
fn web_pages_and_rebound_hosts_are_refused() {
    let (_bridge, port) = bridge();
    let ok = [("X-Nudgy-Secret", SECRET)];
    assert_eq!(heartbeat(port, &[ok[0], ("Origin", "https://evil.example")], CHROME_TAB), 403);
    assert_eq!(heartbeat(port, &[ok[0], ("Host", "evil.example:47615")], CHROME_TAB), 403);
    assert_eq!(heartbeat(port, &[ok[0], ("Origin", "chrome-extension://abcdefgh")], CHROME_TAB), 204);
    assert_eq!(heartbeat(port, &[ok[0], ("Origin", "moz-extension://1234")], CHROME_TAB), 204);
}

#[test]
fn bad_heartbeats_are_rejected() {
    let (_bridge, port) = bridge();
    let ok = [("X-Nudgy-Secret", SECRET)];
    assert_eq!(heartbeat(port, &ok, "not json"), 400);
    assert_eq!(heartbeat(port, &ok, &CHROME_TAB.replace("chrome", "netscape")), 400);
    assert_eq!(heartbeat(port, &ok, &CHROME_TAB.replace("instagram.com", "instagram.com/p/secret")), 400);
    let huge = CHROME_TAB.replace("Instagram", &"x".repeat(10_000));
    assert_eq!(heartbeat(port, &ok, &huge), 413);
    assert_eq!(http(port, "GET", "/v1/other", &[], "").0, 404);
}

#[test]
fn incognito_heartbeats_keep_nothing() {
    let (bridge, port) = bridge();
    let body = r#"{"browser":"edge","domain":"bank.example","title":"Bank","incognito":true,"at":0}"#;
    assert_eq!(heartbeat(port, &[("X-Nudgy-Secret", SECRET)], body), 204);
    let hb = bridge.heartbeat_for("edge", now_ms()).unwrap();
    assert!(hb.incognito && hb.domain.is_empty() && hb.title.is_none());
}

#[test]
fn pairing_hands_out_the_secret_once() {
    let (bridge, port) = bridge();
    assert_eq!(http(port, "GET", "/v1/pair", &[], "").0, 403);
    assert!(bridge.status(now_ms()).pairing_open_until.is_none());

    bridge.open_pairing(now_ms());
    assert!(bridge.status(now_ms()).pairing_open_until.is_some());
    let (status, body) = http(port, "GET", "/v1/pair", &[("Origin", "chrome-extension://abc")], "");
    assert_eq!(status, 200);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap()["secret"], SECRET);
    assert_eq!(http(port, "GET", "/v1/pair", &[], "").0, 403);
    assert!(bridge.status(now_ms()).pairing_open_until.is_none());
}

#[test]
fn status_lists_browsers_that_reported() {
    let (bridge, port) = bridge();
    heartbeat(port, &[("X-Nudgy-Secret", SECRET)], CHROME_TAB);
    let status = bridge.status(now_ms());
    assert!(status.listening);
    assert_eq!(status.browsers.iter().map(|b| b.browser.as_str()).collect::<Vec<_>>(), ["chrome"]);
}

#[test]
fn a_taken_port_falls_back_to_the_next() {
    let (_first, taken) = bridge();
    let second = Bridge::start(&[taken, 0], SECRET.into());
    let port = second.port().expect("fell back");
    assert_ne!(port, taken);
    assert_eq!(heartbeat(port, &[("X-Nudgy-Secret", SECRET)], CHROME_TAB), 204);
}

#[test]
fn no_free_port_is_reported_not_fatal() {
    let (_first, taken) = bridge();
    let second = Bridge::start(&[taken], SECRET.into());
    assert!(!second.status(now_ms()).listening);
    assert!(second.heartbeat_for("chrome", now_ms()).is_none());
}

#[test]
fn hello_identifies_nudgy_on_every_response() {
    let (_bridge, port) = bridge();
    let raw = |path: &str| {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response.to_lowercase()
    };
    let hello = raw("/v1/hello");
    assert!(hello.starts_with("http/1.1 204"), "{hello}");
    assert!(hello.contains("x-nudgy: 1"));
    // Refusals say who they are too, so the extension stops probing other ports.
    assert!(raw("/v1/pair").contains("x-nudgy: 1"));
}
