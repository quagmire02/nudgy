//! Personalised AI lines. The device token and the random install id live
//! in `cloud.key`, protected by DPAPI like the database key. Without a
//! line-writer address (NUDGY_API_URL at build or run time) the panda uses
//! canned lines only.

use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Local;
use nudgy_activity::keys;
use nudgy_cloud::{Cloud, Credentials};
use nudgy_shared::{LineRequest, Persona, RegisterRequest, Settings, Trigger};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    install_id: String,
    token: Option<String>,
}

/// DPAPI-protected `cloud.key`.
struct Stored {
    path: PathBuf,
    saved: Mutex<Saved>,
}

impl Stored {
    fn load(path: PathBuf) -> Self {
        let saved = std::fs::read(&path)
            .ok()
            .and_then(|blob| keys::unprotect(&blob).ok())
            .and_then(|json| serde_json::from_slice::<Saved>(&json).ok())
            .filter(|s| !s.install_id.is_empty())
            .unwrap_or_else(|| Saved { install_id: random_uuid(), token: None });
        let stored = Stored { path, saved: Mutex::new(saved) };
        stored.save();
        stored
    }

    fn save(&self) {
        let json = serde_json::to_vec(&*self.saved.lock().unwrap()).unwrap();
        match keys::protect(&json) {
            Ok(blob) => {
                if let Err(e) = std::fs::write(&self.path, blob) {
                    log::warn!("can't save cloud.key: {e}");
                }
            }
            Err(e) => log::warn!("can't protect cloud.key: {e}"),
        }
    }
}

impl Credentials for Stored {
    fn token(&self) -> Option<String> {
        self.saved.lock().unwrap().token.clone()
    }

    fn set_token(&self, token: Option<&str>) {
        self.saved.lock().unwrap().token = token.map(str::to_string);
        self.save();
    }

    fn registration(&self) -> RegisterRequest {
        RegisterRequest {
            install_id: self.saved.lock().unwrap().install_id.clone(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            utc_offset_min: Local::now().offset().local_minus_utc() / 60,
        }
    }
}

/// A random version-4 UUID: identifies the install, nothing about the PC.
fn random_uuid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("random bytes");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// The configured line writer, if any.
pub fn start(data_dir: PathBuf) -> Option<Cloud> {
    let url = std::env::var("NUDGY_API_URL").ok().or(option_env!("NUDGY_API_URL").map(str::to_string))?;
    let credentials = Stored::load(data_dir.join("cloud.key"));
    match Cloud::new(&url, Box::new(credentials)) {
        Ok(cloud) => {
            log::info!("AI lines from {url}");
            Some(cloud)
        }
        Err(e) => {
            log::warn!("AI lines off: {e}");
            None
        }
    }
}

/// What leaves the PC: the trigger, tone and a few facts. Window titles only
/// if the user opted in.
pub fn request(trigger: &Trigger, settings: &Settings, title: Option<String>) -> LineRequest {
    let cap = |text: &str, max: usize| text.chars().take(max).collect::<String>();
    let c = &trigger.context;
    LineRequest {
        trigger: trigger.kind,
        tone: trigger.tone,
        persona: Persona::Panda,
        language: "en".into(),
        goal: c.goal.as_deref().map(|g| cap(g, 80)),
        category: c.category,
        app: c.app.as_deref().map(|a| cap(a, 40)),
        minutes: c.minutes,
        streak: c.streak,
        nudge_level: c.nudge_level,
        title: title.filter(|_| settings.share_titles_with_cloud).map(|t| cap(&t, 120)),
        swearing: settings.swearing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nudgy_shared::{CharacterState, Tone, TriggerContext, TriggerKind};

    fn trigger() -> Trigger {
        Trigger {
            id: "t".into(),
            kind: TriggerKind::DriftFirst,
            at: 0,
            session_id: None,
            tone: Tone::Coach,
            context: TriggerContext { goal: Some("g".repeat(200)), app: Some("instagram.com".into()), ..Default::default() },
            animation: CharacterState::Alert,
            replies: vec![],
            ai_eligible: true,
        }
    }

    #[test]
    fn titles_leave_only_with_consent_and_text_is_capped() {
        let mut settings = Settings::default();
        let req = request(&trigger(), &settings, Some("Secret tab".into()));
        assert_eq!(req.title, None);
        assert_eq!(req.goal.unwrap().len(), 80);
        settings.share_titles_with_cloud = true;
        assert_eq!(request(&trigger(), &settings, Some("Secret tab".into())).title.as_deref(), Some("Secret tab"));
    }

    #[test]
    fn install_ids_are_random_uuids() {
        let (a, b) = (random_uuid(), random_uuid());
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
    }
}
