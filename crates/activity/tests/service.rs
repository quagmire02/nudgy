//! The whole chain on a real thread: probe -> service -> bridge pairing ->
//! heartbeat -> snapshot -> encrypted store.
#![cfg(windows)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use nudgy_activity::{ActivityService, Probe, Sample, ServiceConfig, WindowSample};
use nudgy_shared::{Foreground, Presence};

struct ChromeInFront;

impl Probe for ChromeInFront {
    fn sample(&mut self) -> Sample {
        Sample {
            window: Some(WindowSample {
                exe: "chrome.exe".into(),
                app_name: "Google Chrome".into(),
                title: "Instagram - Google Chrome".into(),
                shell: false,
                fullscreen: false,
            }),
            ..Sample::default()
        }
    }
}

fn http(port: u16, method: &str, path: &str, extra: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: chrome-extension://test\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split(' ').nth(1).unwrap().parse().unwrap();
    (status, response.split("\r\n\r\n").nth(1).unwrap_or_default().to_string())
}

#[test]
fn paired_extension_turns_chrome_into_a_site() {
    let dir = tempfile::tempdir().unwrap();
    let config = ServiceConfig {
        data_dir: dir.path().to_path_buf(),
        ports: vec![0],
        interval: Duration::from_millis(50),
        private_apps: vec![],
    };
    let mut service = ActivityService::start(config, Box::new(ChromeInFront)).unwrap();
    let port = service.bridge_port().unwrap();
    let updates = service.subscribe();

    // Before pairing: Chrome is just an app.
    let first = updates.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(first.current.foreground, Foreground::App { ref app_id, .. } if app_id == "chrome.exe"));

    // The user clicks "Connect browser"; the extension asks for the secret.
    assert_eq!(http(port, "GET", "/v1/pair", "", "").0, 403);
    service.open_pairing();
    let (status, body) = http(port, "GET", "/v1/pair", "", "");
    assert_eq!(status, 200);
    let secret = serde_json::from_str::<serde_json::Value>(&body).unwrap()["secret"].as_str().unwrap().to_string();
    assert_eq!(secret.len(), 64);

    let heartbeat = r#"{"browser":"chrome","domain":"instagram.com","title":"Instagram","incognito":false,"at":0}"#;
    let headers = format!("Content-Type: application/json\r\nX-Nudgy-Secret: {secret}\r\n");
    assert_eq!(http(port, "POST", "/v1/heartbeat", &headers, heartbeat).0, 204);

    let web = updates
        .iter()
        .find(|u| matches!(u.current.foreground, Foreground::Web { .. }))
        .unwrap();
    assert_eq!(
        web.current.foreground,
        Foreground::Web { browser: "chrome.exe".into(), domain: "instagram.com".into(), title: Some("Instagram".into()) }
    );
    assert_eq!(web.current.presence, Presence::Active);
    assert_eq!(web.ended.len(), 1, "the app event ended when the site was known");
    assert_eq!(service.extension_status().browsers.len(), 1);

    std::thread::sleep(Duration::from_millis(200));
    service.stop();
    let events = service.recent(10).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].end(), events[1].start);
    assert!(events[1].duration >= 200);

    // Pausing records nothing new.
    let mut service = ActivityService::start(
        ServiceConfig { data_dir: dir.path().to_path_buf(), ports: vec![0], interval: Duration::from_millis(50), private_apps: vec![] },
        Box::new(ChromeInFront),
    )
    .unwrap();
    service.set_paused(true);
    std::thread::sleep(Duration::from_millis(300));
    assert!(service.current().is_none());
    service.stop();
    assert_eq!(service.recent(10).unwrap().len(), 2, "same key and data reopened, nothing added while paused");
}
