use anyhow::Result;
use reqwest::blocking::Client as Http;
use serde_json::Value;
use std::time::Duration;

const BASE_URL: &str = "https://public.heypocketai.com/api/v1";

/// POCKET_APP_KEY is used exactly as set. The key file is trimmed, so a file
/// written by `pbpaste` works as-is.
pub fn app_key() -> String {
    match vaultykeys::get_for("pocket", "POCKET_APP_KEY").ok_or(()) {
        Ok(key) if !key.is_empty() => key,
        _ => {
            let file = dirs::home_dir().unwrap_or_default().join(".config").join("pocket").join("key");
            std::fs::read_to_string(file).map(|key| key.trim().to_string()).unwrap_or_default()
        }
    }
}

pub fn recording_endpoint(id: &str) -> String {
    format!("/public/recordings/{id}?include_transcript=true&include_summarizations=true")
}

pub struct Client {
    http: Http,
    key: String,
}

impl Client {
    pub fn new() -> Result<Self> {
        let http = Http::builder().timeout(Duration::from_secs(120)).build()?;
        Ok(Self { http, key: app_key() })
    }

    /// The `data` of one answer. A failure comes back as the ✗ line to print,
    /// so a parallel export can print it inside its own block.
    pub fn get(&self, endpoint: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(format!("{BASE_URL}{endpoint}"))
            .bearer_auth(&self.key)
            .send()
            .map_err(|err| format!("✗ {err}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("✗ {} — {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
        }
        let text = response.text().map_err(|err| format!("✗ {err}"))?;
        let body: Value =
            serde_json::from_str(&text).map_err(|_| format!("✗ {endpoint} did not answer JSON"))?;
        if body.get("success") == Some(&Value::Bool(false)) {
            let error = &body["error"];
            return Err(format!("✗ {}", error.as_str().map_or_else(|| error.to_string(), str::to_string)));
        }
        Ok(body["data"].clone())
    }

    /// One page of 100. pocket does not paginate, so anything past that is
    /// never listed or exported.
    pub fn list_recordings(&self) -> Vec<Value> {
        let endpoint = "/public/recordings?limit=100";
        println!("→ GET {endpoint}");
        match self.get(endpoint) {
            Ok(Value::Array(recs)) => recs,
            Ok(_) => Vec::new(),
            Err(line) => {
                println!("{line}");
                Vec::new()
            }
        }
    }
}
