use crate::auth::ensure_token;
use crate::config::{Config, SingleAccountConfig};
use crate::error::{Result, VgoogError};
use crate::tier::{self, Tier};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use reqwest::{Client, Method, RequestBuilder, Response};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Methods that carry content, so their requests always say how long it is — zero included.
const WITH_CONTENT: [Method; 3] = [Method::POST, Method::PUT, Method::PATCH];

/// Every API request is built here. Google answers a POST that does not state its length with
/// 411 Length Required, and reqwest states none when there is no body — so `trash_thread`,
/// `untrash_message` and every other body-less POST failed that way. The body is encoded up front
/// and its length always sent, so nothing ever goes out chunked or unmeasured.
fn build(http: &Client, method: Method, url: &str, token: &str, body: Option<&Value>) -> Result<RequestBuilder> {
    let req = http.request(method.clone(), url).bearer_auth(token);
    if let Some(b) = body {
        let bytes = serde_json::to_vec(b)?;
        return Ok(req
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bytes.len())
            .body(bytes));
    }
    if !WITH_CONTENT.contains(&method) {
        return Ok(req);
    }
    Ok(req.header(CONTENT_LENGTH, 0))
}

/// Drive's multipart upload. Every part is in memory, so reqwest can measure the form and sends
/// its `Content-Length` instead of chunking it — the test below holds it to that.
fn upload_form(metadata: &Value, file_bytes: Vec<u8>, mime_type: &str) -> Result<Form> {
    let metadata_part = Part::text(serde_json::to_string(metadata)?).mime_str("application/json")?;
    let file_part = Part::bytes(file_bytes).mime_str(mime_type)?;
    Ok(Form::new().part("metadata", metadata_part).part("file", file_part))
}

/// Tries at a rate-limited call, waiting 1, 2, 4, 8 then 16 s between them — about half a minute,
/// inside the 2 minutes an agent's `google` call is given.
const RATE_ATTEMPTS: u32 = 6;

/// A limit that clears in seconds. `userRateLimitExceeded` is spelled with a capital R.
fn rate_limited(result: &Result<Value>) -> bool {
    match result {
        Err(VgoogError::RateLimited { .. }) => true,
        Err(VgoogError::Api { status: 403, message }) => message.contains("rateLimitExceeded") || message.contains("RateLimitExceeded"),
        _ => false,
    }
}

pub struct GoogleClient {
    http: Client,
    /// The account this client acts as — not necessarily the active one. `--account` builds a
    /// client for any account and leaves `active_account` alone.
    account_config: Arc<Mutex<SingleAccountConfig>>,
    /// The full multi-account config for account switching
    full_config: Arc<Mutex<Config>>,
}

impl GoogleClient {
    /// A client for the named account, or for the active one when `account` is None.
    pub fn new(config: Config, account: Option<&str>) -> Result<Self> {
        let name = account.map(str::to_string).unwrap_or_else(|| config.active_account.clone());
        let account_config = config.for_account(&name)?;
        let http = Client::builder()
            .user_agent("vgoog/0.1.0")
            .build()
            .expect("Failed to create HTTP client");
        Ok(Self {
            http,
            account_config: Arc::new(Mutex::new(account_config)),
            full_config: Arc::new(Mutex::new(config)),
        })
    }

    /// A client for another account that shares this one's connections. The active account does
    /// not change.
    pub async fn for_account(&self, name: &str) -> Result<Self> {
        let account_config = self.full_config.lock().await.for_account(name)?;
        Ok(Self {
            http: self.http.clone(),
            account_config: Arc::new(Mutex::new(account_config)),
            full_config: self.full_config.clone(),
        })
    }

    /// Make another account the active one — the TUI's Ctrl+A. Only `active_account` is written,
    /// so a token another vgoog process saved in the meantime survives.
    pub async fn switch_account(&self, name: &str) -> Result<()> {
        let mut full = self.full_config.lock().await;
        let account_config = full.for_account(name)?;
        Config::update(&full, |config| config.active_account = name.to_string())?;
        full.active_account = name.to_string();
        *self.account_config.lock().await = account_config;
        Ok(())
    }

    /// The name of the account this client acts as
    pub async fn active_account_name(&self) -> String {
        self.account_config.lock().await.name.clone()
    }

    /// The label of the account this client acts as
    pub async fn active_account_label(&self) -> String {
        let name = self.active_account_name().await;
        let full = self.full_config.lock().await;
        full.accounts
            .get(&name)
            .map(|a| a.label.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// The tier of the account this client acts as
    pub async fn tier(&self) -> Tier {
        self.account_config.lock().await.tier
    }

    /// Get all account names
    pub async fn account_names(&self) -> Vec<String> {
        self.full_config
            .lock()
            .await
            .account_names()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Get the full config (for account management)
    pub async fn get_full_config(&self) -> Config {
        self.full_config.lock().await.clone()
    }

    /// Update the full config (after adding/removing accounts)
    pub async fn update_full_config(&self, config: Config) -> Result<()> {
        config.save()?;
        let new_account_config = config.for_active_account()?;
        let mut full = self.full_config.lock().await;
        *full = config;
        let mut current = self.account_config.lock().await;
        *current = new_account_config;
        Ok(())
    }

    /// The one door every request goes through: refuse what this account's tier does not allow,
    /// then hand back a token for the rest. The refusal comes first, so a denied call never even
    /// mints a token.
    async fn authorize(&self, url: &str) -> Result<String> {
        let mut config = self.account_config.lock().await;
        tier::check(&config.name, config.tier, &config.scopes, url)?;
        ensure_token(&mut config).await?;
        Ok(config.auth.access_token.clone())
    }

    async fn handle_response(&self, resp: Response) -> Result<Value> {
        let status = resp.status();
        if status.is_success() {
            if status == reqwest::StatusCode::NO_CONTENT {
                return Ok(Value::Null);
            }
            let body = resp.text().await?;
            if body.is_empty() {
                return Ok(Value::Null);
            }
            let val: Value = serde_json::from_str(&body)?;
            Ok(val)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            Err(VgoogError::RateLimited {
                retry_after_secs: retry_after,
            })
        } else if status == reqwest::StatusCode::NOT_FOUND {
            Err(VgoogError::NotFound("Resource not found".into()))
        } else {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(VgoogError::Api {
                status: code,
                message: body,
            })
        }
    }

    /// Google's short-term limits are waited out and the call made again: a 429, or a 403 whose
    /// reason is `rateLimitExceeded`. Accepting a run of calendar invites hit one on the second
    /// call, and the agent gave up with fifteen left. A daily quota (`quotaExceeded`) is not
    /// retried — seconds will not fix it.
    pub async fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let mut wait = std::time::Duration::from_secs(1);
        let mut attempt = 1;
        loop {
            let token = self.authorize(url).await?;
            let req = build(&self.http, method.clone(), url, &token, body)?;
            let result = self.handle_response(req.send().await?).await;
            if attempt >= RATE_ATTEMPTS || !rate_limited(&result) {
                return result;
            }
            tokio::time::sleep(wait).await;
            wait *= 2;
            attempt += 1;
        }
    }

    pub async fn get(&self, url: &str) -> Result<Value> {
        self.request(Method::GET, url, None).await
    }

    pub async fn post(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::POST, url, Some(body)).await
    }

    pub async fn put(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::PUT, url, Some(body)).await
    }

    pub async fn patch(&self, url: &str, body: &Value) -> Result<Value> {
        self.request(Method::PATCH, url, Some(body)).await
    }

    pub async fn delete(&self, url: &str) -> Result<Value> {
        self.request(Method::DELETE, url, None).await
    }

    pub async fn upload_multipart(
        &self,
        url: &str,
        metadata: &Value,
        file_bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<Value> {
        let token = self.authorize(url).await?;
        let form = upload_form(metadata, file_bytes, mime_type)?;
        let resp = self
            .http
            .post(url)
            .bearer_auth(&token)
            .multipart(form)
            .send()
            .await?;
        self.handle_response(resp).await
    }

    pub async fn download(&self, url: &str) -> Result<Vec<u8>> {
        let token = self.authorize(url).await?;
        let resp = self.http.get(url).bearer_auth(&token).send().await?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp.bytes().await?.to_vec())
        } else {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(VgoogError::Api {
                status: code,
                message: body,
            })
        }
    }

    pub async fn post_empty(&self, url: &str) -> Result<Value> {
        self.request(Method::POST, url, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// Read one request off the socket: its head, lowercased, and its body.
    fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
        let mut seen = Vec::new();
        let mut buf = [0u8; 4096];
        let head_end = loop {
            if let Some(i) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0, "connection closed before the head ended");
            seen.extend_from_slice(&buf[..n]);
        };
        let head = String::from_utf8_lossy(&seen[..head_end]).to_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .map_or(0, |v| v.trim().parse().unwrap());
        while seen.len() < head_end + length {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0, "connection closed before the body ended");
            seen.extend_from_slice(&buf[..n]);
        }
        (head, seen[head_end..].to_vec())
    }

    /// Send one request to a local socket and return it as it arrived. What reqwest was asked to
    /// send is not the point — what Google receives is.
    async fn on_the_wire(make: impl FnOnce(&str) -> RequestBuilder) -> (String, Vec<u8>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/gmail/v1/users/me/threads/t1/trash", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}").unwrap();
            request
        });
        let resp = make(&url).send().await.unwrap();
        assert!(resp.status().is_success());
        server.join().unwrap()
    }

    #[tokio::test]
    async fn bodyless_post_states_a_zero_length() {
        let (head, body) = on_the_wire(|url| build(&Client::new(), Method::POST, url, "t", None).unwrap()).await;
        assert!(head.contains("\r\ncontent-length: 0\r\n"), "{head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn json_post_states_its_exact_length() {
        let sent = json!({ "addLabelIds": ["TRASH"], "removeLabelIds": ["INBOX"] });
        let expected = serde_json::to_vec(&sent).unwrap().len();
        let (head, body) = on_the_wire(|url| build(&Client::new(), Method::POST, url, "t", Some(&sent)).unwrap()).await;
        assert!(head.contains(&format!("\r\ncontent-length: {expected}\r\n")), "{head}");
        assert!(!head.contains("transfer-encoding"), "{head}");
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), sent);
    }

    #[tokio::test]
    async fn get_states_no_length() {
        let (head, _) = on_the_wire(|url| build(&Client::new(), Method::GET, url, "t", None).unwrap()).await;
        assert!(!head.contains("content-length"), "{head}");
    }

    #[tokio::test]
    async fn multipart_upload_is_measured_not_chunked() {
        let (head, body) = on_the_wire(|url| {
            let form = upload_form(&json!({ "name": "a.txt" }), b"hello".to_vec(), "text/plain").unwrap();
            Client::new().post(url).multipart(form)
        })
        .await;
        assert!(head.contains("\r\ncontent-length: "), "{head}");
        assert!(!head.contains("transfer-encoding"), "{head}");
        assert!(body.windows(5).any(|w| w == b"hello"));
    }
}
