use crate::error::{Error, Result};
use regex::Regex;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

pub struct Transport {
    client: Client,
    identifier: Mutex<Option<(String, Instant)>>,
    requests: Semaphore,
    cooldown: AtomicU64,
}
impl Transport {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("Reson/0.1.0 (+https://github.com/atlasru/reson)")
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error("Too many redirects");
                }
                if valid_host(
                    attempt.url(),
                    &["soundcloud.com", "sndcdn.com", "soundcloud.cloud"],
                ) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|_| Error::Network)?;
        Ok(Self {
            client,
            identifier: Mutex::new(None),
            requests: Semaphore::new(4),
            cooldown: AtomicU64::new(0),
        })
    }
    async fn public_identifier(&self, cancel: &CancellationToken) -> Result<String> {
        let mut key = tokio::select! { _=cancel.cancelled()=> return Err(Error::Cancelled), k=self.identifier.lock()=>k };
        if let Some((value, time)) = &*key {
            if time.elapsed() < Duration::from_secs(3600) {
                return Ok(value.clone());
            }
        }
        let html = self
            .text("https://soundcloud.com", 2_000_000, cancel)
            .await?;
        static SCRIPTS: OnceLock<Regex> = OnceLock::new();
        static ID: OnceLock<Regex> = OnceLock::new();
        let scripts = SCRIPTS.get_or_init(|| {
            Regex::new(r#"<script[^>]+src="(https://a-v2\.sndcdn\.com/[^"<>]+)""#)
                .expect("fixed regex")
        });
        let id = ID.get_or_init(|| {
            Regex::new(r#"client_id\s*:\s*"([a-zA-Z0-9]{32})""#).expect("fixed regex")
        });
        for script in scripts
            .captures_iter(&html)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(8)
        {
            let body = self.text(&script[1], 8_000_000, cancel).await?;
            if let Some(value) = id.captures(&body) {
                let value = value[1].to_owned();
                *key = Some((value.clone(), Instant::now()));
                return Ok(value);
            }
        }
        Err(Error::Invalid(
            "SoundCloud changed its public interface. Update Reson or try again later.".into(),
        ))
    }
    async fn text(&self, url: &str, limit: usize, cancel: &CancellationToken) -> Result<String> {
        let response = tokio::select! { _=cancel.cancelled()=>return Err(Error::Cancelled), r=self.client.get(url).send()=>r.map_err(|e| { eprintln!("Public resource failure on {}: {:?}", url::Url::parse(url).unwrap().host_str().unwrap(), e.without_url()); Error::Network })? };
        if !response.status().is_success() {
            return Err(Error::Invalid(format!(
                "SoundCloud public resources returned HTTP {}",
                response.status().as_u16()
            )));
        }
        let body = bounded_body(response, limit, cancel).await?;
        String::from_utf8(body).map_err(|_| Error::Malformed)
    }
    pub async fn get(
        &self,
        path: &str,
        params: &[(&str, String)],
        cancel: CancellationToken,
    ) -> Result<Value> {
        if !path.starts_with('/') || path.contains('?') || path.contains("..") {
            return Err(Error::Invalid("Invalid provider endpoint".into()));
        }
        self.request(
            &format!("https://api-v2.soundcloud.com{path}"),
            params,
            &cancel,
        )
        .await
    }
    pub async fn resolve(&self, raw: &str, params: &[(&str, String)]) -> Result<Value> {
        let url = url::Url::parse(raw).map_err(|_| Error::Malformed)?;
        if url.host_str() != Some("api-v2.soundcloud.com")
            || url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error::Malformed);
        }
        self.request(raw, params, &CancellationToken::new()).await
    }
    async fn request(
        &self,
        raw: &str,
        params: &[(&str, String)],
        cancel: &CancellationToken,
    ) -> Result<Value> {
        let _permit = tokio::select! { _=cancel.cancelled()=>return Err(Error::Cancelled), p=self.requests.acquire()=>p.map_err(|_| Error::Cancelled)? };
        let mut refreshed = false;
        for attempt in 0..3 {
            let remaining = self.cooldown.load(Ordering::Relaxed).saturating_sub(now());
            if remaining > 0 {
                return Err(Error::RateLimited(remaining));
            }
            let identifier = self.public_identifier(cancel).await?;
            let mut url = url::Url::parse(raw).map_err(|_| Error::Malformed)?;
            {
                let mut query = url.query_pairs_mut();
                for (k, v) in params {
                    query.append_pair(k, v);
                }
                query.append_pair("client_id", &identifier);
            }
            let started = Instant::now();
            let response = tokio::select! { _=cancel.cancelled()=>return Err(Error::Cancelled), r=self.client.get(url).send()=>r };
            let response = match response {
                Ok(r) => r,
                Err(_) if attempt < 2 => {
                    backoff(attempt, cancel).await?;
                    continue;
                }
                Err(e) => {
                    eprintln!("Request failure: {:?}", e.without_url());
                    return Err(Error::Network);
                }
            };
            let status = response.status();
            // No query, identifier, authorization or transient stream URL reaches diagnostics.
            tracing::debug!(
                status = status.as_u16(),
                elapsed_ms = started.elapsed().as_millis(),
                "provider response"
            );
            match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN if !refreshed => {
                    *self.identifier.lock().await = None;
                    refreshed = true;
                    continue;
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    let seconds = response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(60)
                        .clamp(1, 86400);
                    self.cooldown.store(now() + seconds, Ordering::Relaxed);
                    return Err(Error::RateLimited(seconds));
                }
                StatusCode::NOT_FOUND | StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => {
                    return Err(Error::Unavailable)
                }
                s if s.is_server_error() && attempt < 2 => {
                    backoff(attempt, cancel).await?;
                    continue;
                }
                s if !s.is_success() => {
                    return Err(Error::Invalid(format!(
                        "SoundCloud request returned HTTP {}",
                        s.as_u16()
                    )))
                }
                _ => {}
            }
            let body = bounded_body(response, 8_000_000, cancel).await?;
            return serde_json::from_slice(&body).map_err(|_| Error::Malformed);
        }
        Err(Error::Network)
    }
}
async fn bounded_body(
    mut response: reqwest::Response,
    limit: usize,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(Error::Malformed);
    }
    let mut body = Vec::new();
    loop {
        let chunk = tokio::select! { _=cancel.cancelled()=>return Err(Error::Cancelled), c=response.chunk()=>c.map_err(|_| Error::Network)? };
        let Some(chunk) = chunk else { break };
        if body.len() + chunk.len() > limit {
            return Err(Error::Malformed);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
async fn backoff(attempt: u32, cancel: &CancellationToken) -> Result<()> {
    tokio::select! { _=cancel.cancelled()=>Err(Error::Cancelled), _=tokio::time::sleep(Duration::from_millis(400 * 2u64.pow(attempt)))=>Ok(()) }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn valid_host(url: &url::Url, domains: &[&str]) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|p| p == 443)
        && url.host_str().is_some_and(|host| {
            domains
                .iter()
                .any(|d| host == *d || host.ends_with(&format!(".{d}")))
        })
}
pub fn validate_stream_url(raw: &str) -> Result<()> {
    let u = url::Url::parse(raw).map_err(|_| Error::Malformed)?;
    if !valid_host(&u, &["sndcdn.com", "soundcloud.cloud", "soundcloud.com"]) {
        return Err(Error::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_hosts_are_bounded() {
        for u in [
            "http://cf.sndcdn.com/a",
            "https://sndcdn.com.evil.test/a",
            "https://127.0.0.1/a",
            "https://user@cf.sndcdn.com/a",
            "file:///tmp/a",
        ] {
            assert!(validate_stream_url(u).is_err());
        }
        assert!(validate_stream_url(
            "https://playback.media-streaming.soundcloud.cloud/a/playlist.m3u8?token=transient"
        )
        .is_ok());
    }
}
