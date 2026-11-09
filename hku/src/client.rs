use crate::config::Config;
use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client as Http, RequestBuilder, Response};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, RANGE};
use reqwest::{Method, StatusCode, Url};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::time::Duration;

const API: &str = "https://api.heroku.com";
const V3: &str = "application/vnd.heroku+json; version=3";
const USER_AGENT: &str = concat!("hku/", env!("CARGO_PKG_VERSION"));

pub struct Client {
    http: Http,
    api_key: String,
}

/// One answer from a list endpoint, and the `Next-Range` to send for the page after it.
struct Page {
    value: Value,
    next: Option<String>,
}

impl Client {
    pub fn new(config: &Config) -> Result<Self> {
        if config.api_key.is_empty() {
            bail!(crate::config::missing_help());
        }
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static(V3));
        let http = Http::builder()
            .default_headers(headers)
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { http, api_key: config.api_key.clone() })
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http.request(method, format!("{API}{path}")).bearer_auth(&self.api_key)
    }

    pub fn get(&self, path: &str) -> Result<Value> {
        self.send(Method::GET, path, None)
    }

    pub fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let mut request = self.request(method.clone(), path);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().with_context(|| format!("{method} {path} failed"))?;
        Self::read(response, path)
    }

    /// A GET that follows `Next-Range` until Heroku stops answering 206 or `limit` items are in hand.
    /// Heroku sends 200 a page unless `range` asks for something else, such as
    /// `version ..; max=20, order=desc`. An answer that is one object comes back as it is.
    pub fn get_all(&self, path: &str, range: Option<String>, limit: usize) -> Result<Value> {
        let Page { value, mut next } = self.page(path, range)?;
        let mut items = match value {
            Value::Array(items) => items,
            other => return Ok(other),
        };
        while items.len() < limit {
            let Some(range) = next.take() else { break };
            let page = self.page(path, Some(range))?;
            next = page.next;
            if let Value::Array(more) = page.value {
                items.extend(more);
            }
        }
        items.truncate(limit);
        Ok(Value::Array(items))
    }

    pub fn list(&self, path: &str, range: Option<String>, limit: usize) -> Result<Vec<Value>> {
        match self.get_all(path, range, limit)? {
            Value::Array(items) => Ok(items),
            _ => bail!("{path} answered with one item, not a list"),
        }
    }

    fn page(&self, path: &str, range: Option<String>) -> Result<Page> {
        let mut request = self.request(Method::GET, path);
        if let Some(range) = range {
            request = request.header(RANGE, range);
        }
        let response = request.send().with_context(|| format!("GET {path} failed"))?;
        let next = match response.status() {
            StatusCode::PARTIAL_CONTENT => {
                response.headers().get("Next-Range").and_then(|v| v.to_str().ok()).map(str::to_string)
            }
            _ => None,
        };
        Ok(Page { value: Self::read(response, path)?, next })
    }

    /// Two steps: open a log session, then read the URL it hands back, one line at a time as the
    /// lines arrive. Returns how many lines came.
    pub fn logs(&self, path: &str, options: &Value, tail: bool, mut each: impl FnMut(&str)) -> Result<usize> {
        let session = self.send(Method::POST, path, Some(options))?;
        let url = session
            .get("logplex_url")
            .and_then(Value::as_str)
            .context("Heroku opened a log session but sent no logplex_url")?;
        // The API client's timeout covers the whole body, and a tail has no end.
        let stream = Http::builder()
            .user_agent(USER_AGENT)
            .timeout(if tail { None } else { Some(Duration::from_secs(60)) })
            .build()?;
        // The session URL carries its own signature, so the token stays on api.heroku.com unless
        // a Heroku log host turns the plain request away.
        let mut response = stream.get(url).send().context("Could not read the log session")?;
        if response.status() == StatusCode::UNAUTHORIZED && on_heroku(url) {
            response = stream
                .get(url)
                .bearer_auth(&self.api_key)
                .send()
                .context("Could not read the log session")?;
        }
        if !response.status().is_success() {
            bail!("The log session answered {}", response.status());
        }
        let mut count = 0;
        for line in BufReader::new(response).lines() {
            each(&line?);
            count += 1;
        }
        Ok(count)
    }

    fn read(response: Response, path: &str) -> Result<Value> {
        let status = response.status();
        // Heroku joins one id per hop with commas. The first is the one its support can look up.
        let request_id = response
            .headers()
            .get("Request-Id")
            .and_then(|v| v.to_str().ok())
            .and_then(|ids| ids.split(',').next())
            .unwrap_or("none")
            .to_string();
        let text = response.text().unwrap_or_default();
        if status.is_success() {
            if text.trim().is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text)
                .with_context(|| format!("{path} returned something that is not JSON"));
        }
        let error = serde_json::from_str::<Value>(&text).unwrap_or_default();
        let id = error.get("id").and_then(Value::as_str).unwrap_or_default();
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| text.chars().take(300).collect());
        match (status.as_u16(), id) {
            (401, _) => bail!("Unauthorized — the Heroku token is missing, wrong or expired (request {request_id})"),
            (403, _) => bail!(
                "Forbidden on {path} — {message}\nConfig vars need a read-protected or global token, and /account needs identity (request {request_id})"
            ),
            (404, _) => bail!("Not found: {path} — {message} (request {request_id})"),
            (429, _) | (_, "rate_limit") => bail!(
                "Heroku's rate limit is spent. The pool holds 4,500 calls and refills at about 75 a minute. (request {request_id})"
            ),
            (code, _) => bail!("Heroku answered {code} on {path}: {message} (request {request_id})"),
        }
    }
}

fn on_heroku(url: &str) -> bool {
    let host = Url::parse(url).ok().and_then(|url| url.host_str().map(str::to_lowercase)).unwrap_or_default();
    host == "heroku.com" || host.ends_with(".heroku.com")
}

/// One path segment, escaped, for an app, team or dyno name typed on the command line.
pub fn segment(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
