//! The client against a fake line writer on localhost.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nudgy_cloud::{Cloud, CloudError, Credentials};
use nudgy_shared::{LineRequest, Persona, Quota, RegisterRequest, Tone, TriggerKind};
use tiny_http::{Header, Response, Server};

const NOW: i64 = 1_792_576_800_000;

#[derive(Default)]
struct Memory {
    token: Mutex<Option<String>>,
}

/// The test keeps one handle, the client the other.
struct Shared(Arc<Memory>);

impl Memory {
    fn token(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }
    fn set_token(&self, token: Option<&str>) {
        *self.token.lock().unwrap() = token.map(str::to_string);
    }
}

impl Credentials for Shared {
    fn token(&self) -> Option<String> {
        self.0.token()
    }
    fn set_token(&self, token: Option<&str>) {
        self.0.set_token(token);
    }
    fn registration(&self) -> RegisterRequest {
        RegisterRequest { install_id: "3f9c1e2a-7b4d-4c8e-a1f2-9d6b5e4c3a21".into(), app_version: "0.1.0".into(), utc_offset_min: 360 }
    }
}

/// What the fake server answers to /v1/line: (status, body).
type Reply = Box<dyn Fn(usize, Option<String>) -> (u16, String) + Send + Sync>;

struct Fake {
    url: String,
    registrations: Arc<AtomicUsize>,
    lines: Arc<AtomicUsize>,
}

fn fake(reply: Reply) -> Fake {
    let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
    let url = format!("http://127.0.0.1:{}", server.server_addr().to_ip().unwrap().port());
    let (registrations, lines) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (r, l) = (registrations.clone(), lines.clone());
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            let auth = request.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.to_string());
            let (status, text) = match request.url() {
                "/v1/register" => {
                    let n = r.fetch_add(1, Ordering::SeqCst) + 1;
                    (200, format!(r#"{{"token":"tok-{n}","expiresAt":1824112800000}}"#))
                }
                "/v1/line" => reply(l.fetch_add(1, Ordering::SeqCst) + 1, auth),
                _ => (404, String::new()),
            };
            let header = Header::from_bytes("Content-Type", "application/json").unwrap();
            let _ = request.respond(Response::from_string(text).with_status_code(status).with_header(header));
        }
    });
    Fake { url, registrations, lines }
}

fn request() -> LineRequest {
    LineRequest {
        trigger: TriggerKind::DriftFirst,
        tone: Tone::Coach,
        persona: Persona::Panda,
        language: "en".into(),
        goal: Some("Thesis chapter 3".into()),
        category: None,
        app: Some("instagram.com".into()),
        minutes: Some(6),
        streak: None,
        nudge_level: 1,
        title: None,
        swearing: false,
    }
}

fn ok_line(text: &str, used: u32) -> (u16, String) {
    (200, format!(r#"{{"text":"{text}","source":"ai","quota":{{"used":{used},"limit":20,"resetsAt":{}}}}}"#, NOW + 3_600_000))
}

fn cloud(url: &str) -> (Cloud, Arc<Memory>) {
    let memory = Arc::new(Memory::default());
    (Cloud::new(url, Box::new(Shared(memory.clone()))).unwrap(), memory)
}

#[test]
fn registers_once_then_gets_lines() {
    let f = fake(Box::new(|n, auth| {
        assert_eq!(auth.as_deref(), Some("Bearer tok-1"));
        ok_line("Six minutes of Instagram. Chapter 3 misses you.", n as u32)
    }));
    let (cloud, memory) = cloud(&f.url);
    for _ in 0..3 {
        let line = cloud.line(&request(), NOW).unwrap();
        assert_eq!(line.text, "Six minutes of Instagram. Chapter 3 misses you.");
    }
    assert_eq!(f.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(memory.token().as_deref(), Some("tok-1"));
    assert_eq!(cloud.quota().unwrap().used, 3);
}

#[test]
fn a_refused_token_is_replaced_once() {
    let f = fake(Box::new(|_, auth| {
        if auth.as_deref() == Some("Bearer stale") {
            (401, r#"{"code":"unauthorized","message":"no"}"#.into())
        } else {
            ok_line("Back to it.", 1)
        }
    }));
    let (cloud, memory) = cloud(&f.url);
    memory.set_token(Some("stale"));
    assert_eq!(cloud.line(&request(), NOW).unwrap().text, "Back to it.");
    assert_eq!(memory.token().as_deref(), Some("tok-1"));
}

#[test]
fn a_spent_quota_stops_calls_until_it_resets() {
    let f = fake(Box::new(|_, _| {
        let quota = format!(r#"{{"used":20,"limit":20,"resetsAt":{}}}"#, NOW + 3_600_000);
        (429, format!(r#"{{"code":"quota_exceeded","message":"limit","quota":{quota}}}"#))
    }));
    let (cloud, _) = cloud(&f.url);
    let expected = Quota { used: 20, limit: 20, resets_at: NOW + 3_600_000 };
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::QuotaExceeded(expected)));
    assert_eq!(cloud.line(&request(), NOW + 1_000), Err(CloudError::QuotaExceeded(expected)));
    assert_eq!(f.lines.load(Ordering::SeqCst), 1, "no second call before the reset");
    assert!(cloud.exhausted(NOW));
    assert!(!cloud.exhausted(NOW + 3_600_000));
}

#[test]
fn the_app_filters_ai_lines_even_if_the_server_did_not() {
    let f = fake(Box::new(|_, _| ok_line("You're so stupid.", 1)));
    let (cloud, _) = cloud(&f.url);
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::Upstream));
}

#[test]
fn server_errors_and_rate_limits_are_errors() {
    let f = fake(Box::new(|n, _| match n {
        1 => (502, r#"{"code":"upstream","message":"x"}"#.into()),
        _ => (429, r#"{"code":"rate_limited","message":"x"}"#.into()),
    }));
    let (cloud, _) = cloud(&f.url);
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::Upstream));
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::RateLimited));
}

#[test]
fn a_slow_server_times_out_as_offline() {
    let f = fake(Box::new(|_, _| {
        std::thread::sleep(Duration::from_secs(3));
        ok_line("Too late.", 1)
    }));
    let (cloud, _) = cloud(&f.url);
    let started = std::time::Instant::now();
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::Offline));
    assert!(started.elapsed() < Duration::from_millis(2_500));
}

#[test]
fn nothing_listening_is_offline_and_plain_http_is_refused() {
    let (cloud, _) = cloud("http://127.0.0.1:9");
    assert_eq!(cloud.line(&request(), NOW), Err(CloudError::Offline));
    assert!(matches!(Cloud::new("http://api.example.com", Box::new(Shared(Arc::new(Memory::default())))), Err(CloudError::Insecure)));
}
