//! The api, as mension sees it: where it is, the token it is called with, and one request. Every
//! call goes through `call`, so the token, the timeout and the way a refusal reads live in one place.

use crate::{Fail, FAILED, SETUP, TIMEOUT};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Url;
use serde_json::Value;
use std::time::Duration;

const TOOL: &str = "mension";
const TOKEN: &str = "MENSION_MACHINE_TOKEN";

/// Where the api is, first one set wins. NINJA_API_URL is the daemon's own, so mension follows the
/// daemon to whichever api it is pointed at without being told twice.
const URL_KEYS: [&str; 2] = ["MENSION_API_URL", "NINJA_API_URL"];
const DEFAULT_URL: &str = "https://api.maxima.ninja/v1";

const WAIT: Duration = Duration::from_secs(8);

pub fn base() -> String {
    URL_KEYS.iter().find_map(|key| vaultykeys::get_for(TOOL, key)).unwrap_or_else(|| DEFAULT_URL.to_string())
}

/// The file the token belongs in, written with `~` the way a person would type it.
fn secrets_file() -> String {
    let path = vaultykeys::secrets_dir().join(format!("{TOOL}.env"));
    let home = std::env::var_os("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// The machine token: the environment first, then mension.env, then vaulty's shared keys.
pub fn token() -> Result<String, Fail> {
    vaultykeys::get_for(TOOL, TOKEN)
        .ok_or_else(|| Fail::new(SETUP, format!("missing {TOKEN} in {}", secrets_file())))
}

/// `<api>/artefacts/<slug>/comments/<route>`, the slug encoded as one path segment.
pub fn comments(slug: &str, route: &str) -> Result<Url, Fail> {
    let base = base();
    let mut url = Url::parse(&base)
        .map_err(|e| Fail::new(SETUP, format!("the api url {base:?} does not parse: {e}")))?;
    url.path_segments_mut()
        .map_err(|_| Fail::new(SETUP, format!("the api url {base:?} cannot take a path")))?
        .pop_if_empty()
        .extend(["artefacts", slug, "comments", route]);
    Ok(url)
}

pub fn get(url: Url) -> Result<Value, Fail> {
    call(client()?.get(url))
}

pub fn post(url: Url, body: &Value) -> Result<Value, Fail> {
    call(client()?.post(url).json(body))
}

fn client() -> Result<Client, Fail> {
    Client::builder().timeout(WAIT).build().map_err(|e| Fail::new(FAILED, format!("no http client: {e}")))
}

/// One request. A 2xx answers its JSON body; anything else fails with the api's own words.
fn call(request: RequestBuilder) -> Result<Value, Fail> {
    let response = request.bearer_auth(token()?).send().map_err(unreached)?;
    let status = response.status();
    let text = response.text().map_err(unreached)?;
    let body: Option<Value> = serde_json::from_str(&text).ok();
    if !status.is_success() {
        return Err(Fail::new(FAILED, refusal(status.as_u16(), body.as_ref(), &text)));
    }
    body.ok_or_else(|| {
        Fail::new(FAILED, format!("the api answered {status} with something that is not JSON"))
    })
}

/// No answer: out of time, or never connected.
fn unreached(error: reqwest::Error) -> Fail {
    if error.is_timeout() {
        return Fail::new(TIMEOUT, "timeout");
    }
    Fail::new(FAILED, format!("the api could not be reached: {:#}", anyhow::Error::from(error)))
}

/// What the api said when it refused: its `message`, else the start of whatever it sent.
fn refusal(status: u16, body: Option<&Value>, text: &str) -> String {
    let said = body.and_then(|b| b["message"].as_str()).map(str::to_string);
    let said = said.unwrap_or_else(|| text.trim().chars().take(200).collect());
    if said.is_empty() {
        return format!("the api answered {status}");
    }
    format!("the api answered {status}: {said}")
}
