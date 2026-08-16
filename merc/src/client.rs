//! The one place that talks to Mercury.
//!
//! Every operation is the same request with a different path, so there is one
//! request builder here rather than seventy. What varies — path placeholders,
//! query parameters, body encoding, pagination — is read from the operation's
//! own row in the generated table.

use crate::config::Config;
use crate::ops::{scalar, In, Method, Op, Output, Paging, Payload};
use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

pub struct Client {
    http: reqwest::blocking::Client,
    config: Config,
}

/// A JSON reply, kept both parsed and verbatim: `--json` prints exactly what
/// Mercury sent, so piping into `jq` never sees a number we reformatted.
pub struct Reply {
    pub value: Value,
    pub raw: String,
    /// Mercury answers a repeated idempotency key with 409 and the original
    /// transaction. That is a success worth saying out loud, not an error.
    pub was_duplicate: bool,
}

pub struct Download {
    pub bytes: Vec<u8>,
    pub filename: String,
}

#[derive(Debug)]
pub enum ApiError {
    /// Carries what is known about the token, because "unauthorized" alone
    /// sends you looking in the wrong place.
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    /// 400/422 — Mercury explains these well, so its own words are shown
    Rejected(String),
    RateLimited,
    Other {
        status: u16,
        body: String,
    },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized(advice) => {
                write!(f, "Mercury rejected the token.\n{advice}")
            }
            ApiError::Forbidden(message) => write!(
                f,
                "Forbidden — {message}\n\
                 Tokens are scoped: read-only tokens cannot send money, and Send Money needs its IP allow-listed."
            ),
            ApiError::NotFound(what) => write!(f, "Not found: {what}"),
            ApiError::Rejected(message) => write!(f, "Mercury rejected this: {message}"),
            ApiError::RateLimited => write!(f, "Rate limited — wait a moment and try again."),
            ApiError::Other { status, body } => write!(f, "API error {status}: {body}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl Client {
    pub fn new(config: &Config) -> Result<Self> {
        if config.api_key.is_empty() {
            bail!(crate::config::missing_key_help());
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent(concat!("merc/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { http, config: config.clone() })
    }

    fn url(&self, op: &'static Op, args: &Map<String, Value>) -> Result<String> {
        let base = match op.is_oauth() {
            true => self.config.oauth_url(),
            false => self.config.base_url(),
        };
        Ok(format!("{base}{}", op.resolve_path(args)?))
    }

    /// One request, one response.
    pub fn run(&self, op: &'static Op, args: &Map<String, Value>) -> Result<Reply> {
        let response = self.send(op, args)?;
        let status = response.status().as_u16();
        let text = response.text().unwrap_or_default();

        // 409 on a send is the idempotency key doing its job.
        if status == 409 {
            if let Ok(value) = serde_json::from_str(&text) {
                return Ok(Reply { value, raw: text, was_duplicate: true });
            }
        }
        if !(200..300).contains(&status) {
            return Err(classify(status, &text, op, &self.config).into());
        }
        if text.trim().is_empty() {
            return Ok(Reply { value: Value::Null, raw: String::new(), was_duplicate: false });
        }
        let value = serde_json::from_str(&text)
            .with_context(|| format!("Could not parse the response from {}", op.path))?;
        Ok(Reply { value, raw: text, was_duplicate: false })
    }

    /// Every page of a list, followed to the end.
    ///
    /// Mercury paginates two ways and neither reports a total page count, so the
    /// only reliable stop is a page that comes back short or empty.
    pub fn run_all(&self, op: &'static Op, args: &Map<String, Value>) -> Result<Reply> {
        let Some(key) = op.list_key else {
            return self.run(op, args);
        };
        let noun = op.noun();
        let mut args = args.clone();
        let mut rows: Vec<Value> = Vec::new();

        for _ in 0..1000 {
            let reply = self.run(op, &args)?;
            let page: Vec<Value> = op.rows(&reply.value).cloned().unwrap_or_default();
            let count = page.len();
            rows.extend(page);

            let next = match op.paging {
                Paging::None => None,
                Paging::Offset => match count {
                    0 => None,
                    _ => Some(("offset".to_string(), Value::from(rows.len()))),
                },
                Paging::Cursor => reply.value["page"]["nextPage"]
                    .as_str()
                    .map(|cursor| ("start_after".to_string(), Value::from(cursor))),
            };
            let Some((parameter, value)) = next else {
                break;
            };
            if count == 0 {
                break;
            }
            args.insert(parameter, value);
        }

        // Put the rows back where this operation keeps them, so the printer and
        // `--json` see the same shape a single page would have had.
        let merged = match key.is_empty() {
            true => Value::Array(rows),
            false => Value::Object(Map::from_iter([
                (noun.to_string(), Value::Array(rows.clone())),
                ("total".to_string(), Value::from(rows.len())),
            ])),
        };
        Ok(Reply { raw: merged.to_string(), value: merged, was_duplicate: false })
    }

    /// A PDF. The name comes from Mercury's own Content-Disposition when it
    /// sends one, so a downloaded statement is not called `pdf`.
    pub fn download(&self, op: &'static Op, args: &Map<String, Value>) -> Result<Download> {
        let response = self.send(op, args)?;
        let status = response.status().as_u16();
        let filename = response
            .headers()
            .get("content-disposition")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split("filename=").nth(1))
            .map(|name| name.trim_matches(['"', ' ', ';']).to_string())
            .unwrap_or_else(|| format!("{}.pdf", op.verb));

        let bytes = response.bytes()?.to_vec();
        if !(200..300).contains(&status) {
            return Err(classify(status, &String::from_utf8_lossy(&bytes), op, &self.config).into());
        }
        Ok(Download { bytes, filename })
    }

    fn send(&self, op: &'static Op, args: &Map<String, Value>) -> Result<reqwest::blocking::Response> {
        let url = self.url(op, args)?;
        let method = match op.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };

        let mut request = self.http.request(method, &url).query(&query(op, args));
        request = match op.is_oauth() {
            true => request.basic_auth(&self.config.client_id, Some(&self.config.client_secret)),
            false => request.bearer_auth(&self.config.api_key),
        };
        request = match op.payload {
            Payload::None => request,
            Payload::Json => request.json(&body(op, args)),
            Payload::Form => request.form(&flat_form(op, args)),
            Payload::Multipart => request.multipart(multipart(op, args)?),
        };

        request.send().with_context(|| format!("{} {url} failed", op.method.label()))
    }
}

/// Query parameters, with repeatable ones expanded: `status=a&status=b`.
fn query(op: &'static Op, args: &Map<String, Value>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for param in op.params_in(In::Query) {
        let Some(value) = args.get(param.name) else {
            continue;
        };
        match value.as_array() {
            Some(items) => pairs.extend(items.iter().map(|item| (param.name.to_string(), scalar(item)))),
            None => pairs.push((param.name.to_string(), scalar(value))),
        }
    }
    pairs
}

/// The JSON body: every body parameter that was supplied, and nothing else.
///
/// Absent is not the same as null — `updateTransaction` uses an explicit null to
/// clear a note — so only keys the caller actually gave are included.
///
/// A body passed whole with `--body` is the starting point, and named arguments
/// are laid over it, so one field can be overridden without rewriting the file.
pub fn body(op: &'static Op, args: &Map<String, Value>) -> Value {
    let mut object =
        args.get(crate::ops::RAW_BODY).and_then(|value| value.as_object().cloned()).unwrap_or_default();
    for param in op.params_in(In::Body) {
        if let Some(value) = args.get(param.name) {
            object.insert(param.name.to_string(), value.clone());
        }
    }
    Value::Object(object)
}

fn flat_form(op: &'static Op, args: &Map<String, Value>) -> Vec<(String, String)> {
    op.params_in(In::Body)
        .filter_map(|param| args.get(param.name).map(|value| (param.name.to_string(), scalar(value))))
        .collect()
}

fn multipart(op: &'static Op, args: &Map<String, Value>) -> Result<reqwest::blocking::multipart::Form> {
    let mut form = reqwest::blocking::multipart::Form::new();
    for param in op.params_in(In::Body) {
        let Some(value) = args.get(param.name) else {
            continue;
        };
        if param.ty != "file" {
            form = form.text(param.name.to_string(), scalar(value));
            continue;
        }
        let path = scalar(value);
        form = form.file(param.name.to_string(), &path).with_context(|| format!("Could not read {path}"))?;
    }
    Ok(form)
}

fn classify(status: u16, body: &str, op: &'static Op, config: &Config) -> ApiError {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let message = ["message", "error", "errorCode"]
        .iter()
        .find_map(|key| parsed["errors"][*key].as_str().or_else(|| parsed[*key].as_str()))
        .unwrap_or(body)
        .trim()
        .to_string();

    match status {
        401 => ApiError::Unauthorized(crate::config::token_advice(config)),
        403 => ApiError::Forbidden(message),
        404 => ApiError::NotFound(format!("{} ({})", op.path, message)),
        429 => ApiError::RateLimited,
        400 | 409 | 413 | 415 | 422 => ApiError::Rejected(message),
        _ => ApiError::Other { status, body: body.to_string() },
    }
}

/// Binary operations must not have their bytes parsed as JSON.
pub fn is_download(op: &'static Op) -> bool {
    op.output == Output::Binary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::find;
    use serde_json::json;

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn repeatable_filters_become_repeated_parameters() {
        let op = find("cards", "list").unwrap();
        let pairs = query(op, &args(json!({"status": ["active", "frozen"], "limit": 5})));
        assert_eq!(
            pairs,
            vec![
                ("status".into(), "active".into()),
                ("status".into(), "frozen".into()),
                ("limit".into(), "5".into())
            ]
        );
    }

    #[test]
    fn path_and_query_arguments_stay_out_of_the_body() {
        let op = find("accounts", "create-transaction").unwrap();
        let sent = body(op, &args(json!({"accountId": "acc_1", "recipientId": "rec_1", "amount": 10.5})));
        assert_eq!(sent, json!({"recipientId": "rec_1", "amount": 10.5}));
    }

    #[test]
    fn an_explicit_null_is_sent_and_an_absent_field_is_not() {
        let op = find("transactions", "update").unwrap();
        let sent = body(op, &args(json!({"transactionId": "t1", "note": null})));
        assert_eq!(sent, json!({"note": null}), "clearing a note needs the null to survive");
        assert_eq!(body(op, &args(json!({"transactionId": "t1"}))), json!({}));
    }

    #[test]
    fn the_amount_on_the_wire_is_the_amount_that_was_typed() {
        // The request is serialised here, not by the code that parsed the flag,
        // so this is the assertion that actually covers what Mercury receives.
        let op = find("accounts", "create-transaction").unwrap();
        let amount = crate::money::Money::parse("1,234.56").unwrap();
        let sent = body(op, &args(json!({"recipientId": "r", "amount": amount.to_api()})));
        assert_eq!(serde_json::to_string(&sent).unwrap(), r#"{"amount":1234.56,"recipientId":"r"}"#);
    }

    #[test]
    fn errors_carry_mercurys_own_words() {
        let op = find("accounts", "list").unwrap();
        let body = r#"{"errors":{"errorCode":"noTokenInDB","message":"No matching token found"}}"#;
        let config = Config { api_key: "secret-token:mercury_production_x".into(), ..Config::default() };
        assert!(matches!(classify(401, body, op, &config), ApiError::Unauthorized(_)));
        match classify(400, body, op, &config) {
            ApiError::Rejected(message) => assert_eq!(message, "No matching token found"),
            other => panic!("expected Rejected, got {other:?}"),
        }
        assert!(classify(403, body, op, &config).to_string().contains("IP allow-listed"));
    }

    #[test]
    fn an_unrecognised_failure_keeps_the_whole_body() {
        let op = find("accounts", "list").unwrap();
        match classify(500, "upstream exploded", op, &Config::default()) {
            ApiError::Other { status, body } => {
                assert_eq!(status, 500);
                assert_eq!(body, "upstream exploded");
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
