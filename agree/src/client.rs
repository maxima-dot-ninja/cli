use crate::config::{Config, BASE_URL};
use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A validation failure the API reported per field, e.g. {"due_at": ["can't be blank"]}.
///
/// Kept structured rather than flattened into a message so the form layer can
/// re-prompt exactly the fields that failed instead of restarting the whole thing.
#[derive(Debug, Clone)]
pub struct FieldErrors(pub BTreeMap<String, Vec<String>>);

impl FieldErrors {
    pub fn message(&self) -> String {
        self.0
            .iter()
            .map(|(field, msgs)| format!("  {field}: {}", msgs.join(", ")))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug)]
pub enum ApiError {
    /// 422/400 with a field map — recoverable, hand back to the form
    Validation(FieldErrors),
    /// 401 — key missing or wrong
    Unauthorized,
    /// 403 — usually a feature not enabled on the org (e.g. customers)
    Forbidden(String),
    NotFound(String),
    /// Anything else, with whatever the server said
    Other { status: u16, body: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Validation(errors) => write!(f, "The API rejected this:\n{}", errors.message()),
            ApiError::Unauthorized => write!(f, "Unauthorized — check your API key"),
            ApiError::Forbidden(msg) => write!(
                f,
                "Forbidden — {msg}\nThis often means the feature is not enabled for your organization."
            ),
            ApiError::NotFound(what) => write!(f, "Not found: {what}"),
            ApiError::Other { status, body } => write!(f, "API error {status}: {body}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub struct Client {
    http: reqwest::Client,
    api_key: String,
}

/// One page of a list response, plus whatever paging metadata came back.
pub struct Page<T> {
    pub items: Vec<T>,
    #[allow(dead_code)]
    pub page: u32,
    pub had_full_page: bool,
}

impl Client {
    pub fn new(config: &Config) -> Result<Self> {
        if config.api_key.is_empty() {
            bail!(crate::config::missing_key_help());
        }
        Ok(Self { http: reqwest::Client::new(), api_key: config.api_key.clone() })
    }

    fn url(path: &str) -> String {
        format!("{BASE_URL}{path}")
    }

    pub async fn get_raw(&self, path: &str, query: &[(String, String)]) -> Result<Value> {
        let response = self
            .http
            .get(Self::url(path))
            .bearer_auth(&self.api_key)
            .query(query)
            .send()
            .await
            .with_context(|| format!("GET {path} failed"))?;
        Self::read(response, path).await
    }

    pub async fn post_raw<B: Serialize>(&self, path: &str, body: &B) -> Result<Value> {
        let response = self
            .http
            .post(Self::url(path))
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path} failed"))?;
        Self::read(response, path).await
    }

    pub async fn patch_raw<B: Serialize>(&self, path: &str, body: &B) -> Result<Value> {
        let response = self
            .http
            .patch(Self::url(path))
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .with_context(|| format!("PATCH {path} failed"))?;
        Self::read(response, path).await
    }

    pub async fn delete_raw(&self, path: &str) -> Result<Value> {
        let response = self
            .http
            .delete(Self::url(path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .with_context(|| format!("DELETE {path} failed"))?;
        Self::read(response, path).await
    }

    /// Every list response wraps its rows in `data`.
    pub async fn list<T: DeserializeOwned>(
        &self,
        path: &str,
        page: u32,
        page_size: u32,
        filters: &[(String, String)],
    ) -> Result<Page<T>> {
        let mut query = vec![
            ("page".to_string(), page.to_string()),
            ("page_size".to_string(), page_size.to_string()),
        ];
        query.extend_from_slice(filters);

        let body = self.get_raw(path, &query).await?;
        let rows = body.get("data").cloned().unwrap_or(Value::Array(vec![]));
        let items: Vec<T> = serde_json::from_value(rows)
            .with_context(|| format!("Unexpected response shape from {path}"))?;

        let had_full_page = items.len() as u32 >= page_size;
        Ok(Page { items, page, had_full_page })
    }

    /// Walk pages until the API stops filling them. `limit` of 0 means everything.
    pub async fn list_all<T: DeserializeOwned>(
        &self,
        path: &str,
        filters: &[(String, String)],
        limit: usize,
    ) -> Result<Vec<T>> {
        const PAGE_SIZE: u32 = 100;
        let mut all: Vec<T> = Vec::new();
        let mut page = 1;

        loop {
            let chunk = self.list::<T>(path, page, PAGE_SIZE, filters).await?;
            let was_full = chunk.had_full_page;
            all.extend(chunk.items);

            let hit_limit = limit > 0 && all.len() >= limit;
            if !was_full || hit_limit || page >= 100 {
                break;
            }
            page += 1;
        }

        if limit > 0 {
            all.truncate(limit);
        }
        Ok(all)
    }

    async fn read(response: reqwest::Response, path: &str) -> Result<Value> {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if status.is_success() {
            if text.trim().is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_str(&text)
                .with_context(|| format!("Could not parse response from {path}"));
        }

        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        Err(classify(status.as_u16(), parsed, &text, path).into())
    }
}

fn classify(status: u16, parsed: Value, raw: &str, path: &str) -> ApiError {
    if status == 401 {
        return ApiError::Unauthorized;
    }
    if status == 404 {
        return ApiError::NotFound(path.to_string());
    }

    let field_errors = parsed.get("errors").and_then(|e| e.as_object()).map(|map| {
        FieldErrors(
            map.iter()
                .map(|(k, v)| {
                    let msgs = v
                        .as_array()
                        .map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect())
                        .unwrap_or_else(|| vec![v.to_string()]);
                    (k.clone(), msgs)
                })
                .collect(),
        )
    });

    if status == 403 {
        let detail = field_errors.map(|e| e.message()).unwrap_or_else(|| raw.to_string());
        return ApiError::Forbidden(detail);
    }
    match field_errors {
        Some(errors) => ApiError::Validation(errors),
        None => ApiError::Other { status, body: raw.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_errors_survive_classification() {
        let body = json!({"errors": {"due_at": ["can't be blank"], "amount": ["must be positive"]}});
        match classify(422, body, "", "/x") {
            ApiError::Validation(errors) => {
                assert_eq!(errors.0["due_at"], vec!["can't be blank"]);
                assert!(errors.message().contains("amount: must be positive"));
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn forbidden_explains_the_usual_cause() {
        let msg = classify(403, Value::Null, "no access", "/x").to_string();
        assert!(msg.contains("not enabled"), "403 should hint at the feature flag: {msg}");
    }

    #[test]
    fn unauthorized_and_missing_are_distinct() {
        assert!(matches!(classify(401, Value::Null, "", "/x"), ApiError::Unauthorized));
        assert!(matches!(classify(404, Value::Null, "", "/inv"), ApiError::NotFound(_)));
    }

    #[test]
    fn unknown_shapes_keep_the_body() {
        match classify(500, Value::Null, "boom", "/x") {
            ApiError::Other { status, body } => {
                assert_eq!(status, 500);
                assert_eq!(body, "boom");
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
