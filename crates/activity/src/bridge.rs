//! Local HTTP endpoint for the browser extension, on 127.0.0.1 only, on the
//! first free port of `EXTENSION_PORTS`. Every response carries `X-Nudgy: 1`,
//! so the extension never mistakes another local server for Nudgy.
//!
//! - `GET /v1/hello`: 204; lets the extension find which port the app chose.
//! - `POST /v1/heartbeat`: a `BrowserHeartbeat`; needs the `X-Nudgy-Secret` header.
//! - `GET /v1/pair`: hands the secret to the first caller while pairing is
//!   open (the user clicked "Connect browser"), then closes pairing.
//!
//! Requests must address `127.0.0.1:{port}` (blocks DNS rebinding) and, when
//! they carry an `Origin`, come from an extension (blocks web pages).

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use nudgy_shared::{BrowserHeartbeat, ExtensionStatus, SeenBrowser, UnixMs};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::now_ms;
use crate::snapshot::TITLE_MAX_STORED_CHARS;

/// A heartbeat older than this no longer describes the browser (the extension
/// sends a keep-alive every 30 seconds).
pub const HEARTBEAT_FRESH_MS: UnixMs = 90_000;
/// How long "Connect browser" keeps pairing open.
pub const PAIRING_WINDOW_MS: UnixMs = 5 * 60_000;
const MAX_BODY_BYTES: usize = 4096;
const BROWSERS: [&str; 5] = ["chrome", "edge", "firefox", "brave", "opera"];

pub struct Bridge {
    shared: Arc<Shared>,
    server: Option<Arc<Server>>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    secret: String,
    host: String,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Per browser: latest heartbeat and when it arrived (our clock).
    latest: HashMap<String, (BrowserHeartbeat, UnixMs)>,
    pairing_until: Option<UnixMs>,
}

impl Bridge {
    /// Listens on the first of `ports` it can bind on 127.0.0.1 (`0` picks any
    /// free port). If none is free the bridge still exists but reports
    /// `listening: false`, and browsers count as plain apps.
    pub fn start(ports: &[u16], secret: String) -> Bridge {
        let Some(server) = ports.iter().find_map(|&port| match Server::http(("127.0.0.1", port)) {
            Ok(server) => Some(Arc::new(server)),
            Err(e) => {
                log::warn!("browser bridge: can't listen on 127.0.0.1:{port}: {e}");
                None
            }
        }) else {
            let shared = Arc::new(Shared { secret, host: String::new(), state: Mutex::default() });
            return Bridge { shared, server: None, thread: None };
        };
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or_default();
        let shared = Arc::new(Shared { secret, host: format!("127.0.0.1:{port}"), state: Mutex::default() });
        let thread = {
            let (server, shared) = (server.clone(), shared.clone());
            std::thread::Builder::new()
                .name("nudgy-bridge".into())
                .spawn(move || {
                    for request in server.incoming_requests() {
                        handle(&shared, request);
                    }
                })
                .expect("spawn bridge thread")
        };
        Bridge { shared, server: Some(server), thread: Some(thread) }
    }

    pub fn port(&self) -> Option<u16> {
        self.server.as_ref().and_then(|s| s.server_addr().to_ip()).map(|a| a.port())
    }

    /// The browser's latest heartbeat, if still fresh at `now`.
    pub fn heartbeat_for(&self, browser: &str, now: UnixMs) -> Option<BrowserHeartbeat> {
        let state = self.shared.state.lock().unwrap();
        state
            .latest
            .get(browser)
            .filter(|(_, received)| now - received <= HEARTBEAT_FRESH_MS)
            .map(|(hb, _)| hb.clone())
    }

    pub fn open_pairing(&self, now: UnixMs) {
        self.shared.state.lock().unwrap().pairing_until = Some(now + PAIRING_WINDOW_MS);
    }

    pub fn status(&self, now: UnixMs) -> ExtensionStatus {
        let state = self.shared.state.lock().unwrap();
        let mut browsers: Vec<SeenBrowser> = state
            .latest
            .iter()
            .map(|(browser, (_, seen))| SeenBrowser { browser: browser.clone(), last_seen: *seen })
            .collect();
        browsers.sort_by(|a, b| a.browser.cmp(&b.browser));
        ExtensionStatus {
            listening: self.server.is_some(),
            pairing_open_until: state.pairing_until.filter(|until| *until > now),
            browsers,
        }
    }

    pub fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.unblock();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle(shared: &Shared, mut request: Request) {
    let (status, body) = route(shared, &mut request, now_ms());
    let mut response = Response::from_string(body.unwrap_or_default()).with_status_code(status);
    response.add_header(Header::from_bytes("X-Nudgy", "1").unwrap());
    if status == 200 {
        response.add_header(Header::from_bytes("Content-Type", "application/json").unwrap());
    }
    let _ = request.respond(response);
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
}

fn route(shared: &Shared, request: &mut Request, now: UnixMs) -> (u16, Option<String>) {
    if header(request, "Host") != Some(shared.host.as_str()) {
        return (403, None);
    }
    if let Some(origin) = header(request, "Origin")
        && !(origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://"))
    {
        return (403, None);
    }

    match (request.method(), request.url()) {
        (Method::Get, "/v1/hello") => (204, None),
        (Method::Post, "/v1/heartbeat") => {
            let secret = header(request, "X-Nudgy-Secret").unwrap_or_default();
            if !constant_time_eq(secret.as_bytes(), shared.secret.as_bytes()) {
                return (401, None);
            }
            if request.body_length().is_some_and(|len| len > MAX_BODY_BYTES) {
                return (413, None);
            }
            let mut body = Vec::new();
            if request.as_reader().take(MAX_BODY_BYTES as u64 + 1).read_to_end(&mut body).is_err()
                || body.len() > MAX_BODY_BYTES
            {
                return (413, None);
            }
            let Ok(mut hb) = serde_json::from_slice::<BrowserHeartbeat>(&body) else {
                return (400, None);
            };
            if !BROWSERS.contains(&hb.browser.as_str())
                || hb.domain.len() > 253
                || hb.domain.contains(['/', '?', '#', ' '])
            {
                return (400, None);
            }
            if hb.incognito {
                hb.domain.clear();
                hb.title = None;
            }
            hb.title = hb.title.map(|t| t.chars().take(TITLE_MAX_STORED_CHARS).collect());
            let mut state = shared.state.lock().unwrap();
            state.latest.insert(hb.browser.clone(), (hb, now));
            (204, None)
        }
        (Method::Get, "/v1/pair") => {
            let mut state = shared.state.lock().unwrap();
            if state.pairing_until.is_some_and(|until| until > now) {
                state.pairing_until = None;
                (200, Some(serde_json::json!({ "secret": shared.secret }).to_string()))
            } else {
                (403, None)
            }
        }
        _ => (404, None),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
