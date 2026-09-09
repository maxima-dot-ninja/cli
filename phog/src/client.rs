use crate::config::Config;
use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client as Http, Response};
use reqwest::Method;
use serde_json::{json, Value};

/// One HogQL answer: the column names and the rows, as PostHog returns them.
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

pub struct Client {
    http: Http,
    api_key: String,
    host: String,
    project_id: String,
}

impl Client {
    pub fn new(config: &Config) -> Result<Self> {
        if config.api_key.is_empty() || config.project_id.is_empty() {
            bail!(crate::config::missing_help());
        }
        let http = Http::builder().timeout(std::time::Duration::from_secs(120)).build()?;
        Ok(Self {
            http,
            api_key: config.api_key.clone(),
            host: config.host.clone(),
            project_id: config.project_id.clone(),
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Every project-scoped path: `phog` only ever talks about one project.
    pub fn project_path(&self, suffix: &str) -> String {
        format!("/api/projects/{}/{}", self.project_id, suffix.trim_start_matches('/'))
    }

    /// The person page in the app, for a link the terminal can print.
    pub fn person_url(&self, distinct_id: &str) -> String {
        format!("{}/project/{}/person/{}", self.host, self.project_id, urlencode(distinct_id))
    }

    pub fn dashboard_url(&self, id: u64) -> String {
        format!("{}/project/{}/dashboard/{}", self.host, self.project_id, id)
    }

    pub fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        let response = self
            .http
            .get(format!("{}{}", self.host, path))
            .bearer_auth(&self.api_key)
            .query(query)
            .send()
            .with_context(|| format!("GET {path} failed"))?;
        Self::read(response, path)
    }

    pub fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let mut request =
            self.http.request(method.clone(), format!("{}{}", self.host, path)).bearer_auth(&self.api_key);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().with_context(|| format!("{method} {path} failed"))?;
        Self::read(response, path)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::POST, path, Some(body))
    }

    pub fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::PATCH, path, Some(body))
    }

    /// Walks `next` until the list runs out, because PostHog pages everything at 100.
    pub fn list_all(&self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut page = self.get(path, query)?;
        loop {
            if let Some(results) = page.get("results").and_then(Value::as_array) {
                items.extend(results.iter().cloned());
            } else if let Some(array) = page.as_array() {
                items.extend(array.iter().cloned());
                break;
            }
            let next = page.get("next").and_then(Value::as_str).map(str::to_string);
            match next {
                Some(url) => {
                    let path = url.strip_prefix(&self.host).unwrap_or(&url).to_string();
                    page = self.get(&path, &[])?;
                }
                None => break,
            }
        }
        Ok(items)
    }

    /// HogQL, straight through. The query endpoint is the one PostHog API that
    /// sees every event regardless of which person it landed on.
    pub fn query(&self, hogql: &str) -> Result<QueryResult> {
        let body = json!({ "query": { "kind": "HogQLQuery", "query": hogql } });
        let answer = self.post(&self.project_path("query/"), &body)?;
        let columns = answer
            .get("columns")
            .and_then(Value::as_array)
            .map(|cols| cols.iter().map(|c| c.as_str().unwrap_or_default().to_string()).collect())
            .unwrap_or_default();
        let rows = answer
            .get("results")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter().map(|row| row.as_array().cloned().unwrap_or_else(|| vec![row.clone()])).collect()
            })
            .unwrap_or_default();
        Ok(QueryResult { columns, rows })
    }

    fn read(response: Response, path: &str) -> Result<Value> {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if status.is_success() {
            if text.trim().is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text)
                .with_context(|| format!("{path} returned something that is not JSON"));
        }
        let detail = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("detail")
                    .or_else(|| v.get("error"))
                    .or_else(|| v.get("message"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| text.chars().take(300).collect());
        match status.as_u16() {
            401 => bail!("Unauthorized — the personal API key is missing or wrong"),
            403 => bail!("Forbidden — {detail}\nThe key may lack a scope this call needs, or the project id is not yours"),
            404 => bail!("Not found: {path}"),
            code => bail!("PostHog answered {code} on {path}: {detail}"),
        }
    }
}

/// Just enough escaping for a value inside a HogQL string literal.
pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn urlencode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
