//! Milestone 5 (app side): personalised lines from the cloud proxy.
//!
//! Registers the install once for a signed device token, then asks for lines
//! with a short timeout. Every failure (offline, quota, filter) is an error
//! the caller answers with a canned line, so the panda never waits on the
//! network. HTTPS only, except localhost during development; certificates
//! are always verified (rustls, bundled roots).

use std::sync::Mutex;
use std::time::Duration;

use nudgy_shared::{ApiError, ApiErrorCode, LineRequest, LineResponse, Quota, RegisterRequest, RegisterResponse, UnixMs};
use ureq::Agent;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CloudError {
    #[error("the API address must be https:// (http:// only for localhost)")]
    Insecure,
    #[error("no network or the line writer didn't answer in time")]
    Offline,
    #[error("the device token was refused")]
    Unauthorized,
    #[error("daily AI line limit reached")]
    QuotaExceeded(Quota),
    #[error("too many requests")]
    RateLimited,
    #[error("the line writer failed or its line was filtered")]
    Upstream,
    #[error("the request was rejected")]
    BadRequest,
}

/// Where the device token and install id are kept (DPAPI-protected in the app).
pub trait Credentials: Send + Sync {
    fn token(&self) -> Option<String>;
    fn set_token(&self, token: Option<&str>);
    /// This install's random id and UTC offset, for registration.
    fn registration(&self) -> RegisterRequest;
}

pub struct Cloud {
    base: String,
    agent: Agent,
    credentials: Box<dyn Credentials>,
    quota: Mutex<Option<Quota>>,
}

const LINE_TIMEOUT: Duration = Duration::from_millis(1500);
const REGISTER_TIMEOUT: Duration = Duration::from_secs(5);

fn is_local(url: &str) -> bool {
    let rest = url.strip_prefix("http://").unwrap_or("");
    let host = rest.split(['/', ':']).next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

impl Cloud {
    pub fn new(base_url: &str, credentials: Box<dyn Credentials>) -> Result<Self, CloudError> {
        let base = base_url.trim_end_matches('/').to_string();
        if !(base.starts_with("https://") || is_local(&base)) {
            return Err(CloudError::Insecure);
        }
        let agent: Agent = Agent::config_builder().http_status_as_error(false).build().into();
        Ok(Cloud { base, agent, credentials, quota: Mutex::new(None) })
    }

    /// The last quota the server reported.
    pub fn quota(&self) -> Option<Quota> {
        *self.quota.lock().unwrap()
    }

    /// True if the daily quota is known to be spent until its reset.
    pub fn exhausted(&self, now: UnixMs) -> bool {
        self.quota().is_some_and(|q| q.used >= q.limit && now < q.resets_at)
    }

    /// A personalised line that also passed the local content filter.
    pub fn line(&self, request: &LineRequest, now: UnixMs) -> Result<LineResponse, CloudError> {
        if let Some(q) = self.quota().filter(|_| self.exhausted(now)) {
            return Err(CloudError::QuotaExceeded(q));
        }
        match self.try_line(request) {
            // The token was revoked or the server's secret changed: register again, once.
            Err(CloudError::Unauthorized) => {
                self.credentials.set_token(None);
                self.try_line(request)
            }
            other => other,
        }
    }

    fn try_line(&self, request: &LineRequest) -> Result<LineResponse, CloudError> {
        let token = match self.credentials.token() {
            Some(token) => token,
            None => self.register()?,
        };
        let body = serde_json::to_string(request).map_err(|_| CloudError::BadRequest)?;
        let (status, text) = self.post("/v1/line", &body, Some(&token), LINE_TIMEOUT)?;
        if status == 200 {
            let response: LineResponse = serde_json::from_str(&text).map_err(|_| CloudError::Upstream)?;
            *self.quota.lock().unwrap() = response.quota;
            // Filtered on the server too; the app never trusts that alone.
            if nudgy_lines::filter::check(&response.text, request.swearing).is_err() {
                return Err(CloudError::Upstream);
            }
            return Ok(response);
        }
        let error: Option<ApiError> = serde_json::from_str(&text).ok();
        if let Some(q) = error.as_ref().and_then(|e| e.quota) {
            *self.quota.lock().unwrap() = Some(q);
        }
        Err(match (status, error.map(|e| e.code)) {
            (401, _) => CloudError::Unauthorized,
            (429, Some(ApiErrorCode::QuotaExceeded)) => CloudError::QuotaExceeded(self.quota().unwrap_or(Quota {
                used: 0,
                limit: 0,
                resets_at: 0,
            })),
            (429, _) => CloudError::RateLimited,
            (400, _) => CloudError::BadRequest,
            _ => CloudError::Upstream,
        })
    }

    fn register(&self) -> Result<String, CloudError> {
        let body = serde_json::to_string(&self.credentials.registration()).map_err(|_| CloudError::BadRequest)?;
        let (status, text) = self.post("/v1/register", &body, None, REGISTER_TIMEOUT)?;
        match status {
            200 => {
                let response: RegisterResponse = serde_json::from_str(&text).map_err(|_| CloudError::Upstream)?;
                self.credentials.set_token(Some(&response.token));
                Ok(response.token)
            }
            429 => Err(CloudError::RateLimited),
            400 => Err(CloudError::BadRequest),
            _ => Err(CloudError::Upstream),
        }
    }

    fn post(&self, path: &str, body: &str, token: Option<&str>, timeout: Duration) -> Result<(u16, String), CloudError> {
        let mut request = self
            .agent
            .post(format!("{}{path}", self.base))
            .config()
            .timeout_global(Some(timeout))
            .build()
            .content_type("application/json");
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let mut response = request.send(body).map_err(|e| {
            log::info!("cloud request failed: {e}");
            CloudError::Offline
        })?;
        let status = response.status().as_u16();
        let text = response.body_mut().with_config().limit(16 * 1024).read_to_string().map_err(|_| CloudError::Offline)?;
        Ok((status, text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_localhost() {
        assert!(is_local("http://127.0.0.1:8787"));
        assert!(is_local("http://localhost:8787/v1"));
        assert!(!is_local("http://example.com"));
        assert!(!is_local("http://localhost.evil.com"));
    }
}
