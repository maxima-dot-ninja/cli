// The OAuth login flow, done properly: browser out, loopback back.
//
// vgoog used to ask the owner to paste a refresh token fetched by hand from the OAuth Playground.
// That is how you end up with a token issued to Google's client rather than your own, failing with
// `unauthorized_client`, and it is why a Desktop OAuth client kept throwing `redirect_uri_mismatch`
// — the Playground's redirect URI can never be registered on one.
//
// A loopback redirect is what Desktop clients are FOR. We bind a port, send the browser to Google
// with `redirect_uri=http://127.0.0.1:<port>`, and Google hands the code straight back to us. No
// Playground, no paste, no redirect URI to register, and the token belongs to the right client.

use crate::error::{Result, VgoogError};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Long enough to find the browser window and pick an account; short enough that a forgotten
/// login does not leave a socket open all afternoon.
const WAIT_SECS: u64 = 300;

pub struct Granted {
    pub refresh_token: String,
    pub access_token: String,
    pub expiry: DateTime<Utc>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
}

fn page(title: &str, detail: &str) -> String {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>vgoog</title>\
         <body style=\"font:14px ui-monospace,monospace;background:#111;color:#eee;display:grid;place-items:center;height:100vh;margin:0\">\
         <div style=\"text-align:center\"><h1 style=\"font-size:15px;letter-spacing:.2em;text-transform:uppercase\">{title}</h1>\
         <p style=\"color:#888\">{detail}</p></div>"
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Pull the query parameters out of the one request Google makes to us.
fn params_from(request: &str) -> HashMap<String, String> {
    let Some(line) = request.lines().next() else { return HashMap::new() };
    let Some(target) = line.split_whitespace().nth(1) else { return HashMap::new() };
    let Some((_, query)) = target.split_once('?') else { return HashMap::new() };

    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| {
            let decoded = urlencoding::decode(value).map(|v| v.into_owned()).unwrap_or_else(|_| value.to_string());
            (key.to_string(), decoded)
        })
        .collect()
}

/// Serve exactly one request — the redirect — and return whatever Google put in the query.
async fn wait_for_code(listener: TcpListener) -> Result<String> {
    let accept = async {
        loop {
            let (mut socket, _) = listener.accept().await?;

            let mut buffer = vec![0u8; 8192];
            let read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let params = params_from(&request);

            // Browsers ask for /favicon.ico off their own bat; ignore anything that is not the
            // redirect rather than treating it as a failed login.
            if params.is_empty() {
                let _ = socket.write_all(page("vgoog", "waiting for Google…").as_bytes()).await;
                let _ = socket.shutdown().await;
                continue;
            }

            if let Some(error) = params.get("error") {
                let _ = socket.write_all(page("Refused", error).as_bytes()).await;
                let _ = socket.shutdown().await;
                return Err(VgoogError::Auth(format!("Google refused the login: {error}")));
            }

            let Some(code) = params.get("code").cloned() else {
                let _ = socket.write_all(page("Confused", "no code in that redirect").as_bytes()).await;
                let _ = socket.shutdown().await;
                return Err(VgoogError::Auth("the redirect carried no authorization code".into()));
            };

            let _ = socket.write_all(page("Connected", "You can close this tab and go back to the terminal.").as_bytes()).await;
            let _ = socket.shutdown().await;
            return Ok(code);
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(WAIT_SECS), accept).await {
        Ok(result) => result,
        Err(_) => Err(VgoogError::Auth(format!("gave up waiting for the browser after {WAIT_SECS}s"))),
    }
}

fn open_browser(url: &str) -> bool {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(url).status().map(|status| status.success()).unwrap_or(false)
}

/// Run the whole login: bind, open the browser, catch the redirect, exchange the code.
///
/// `on_url` is handed the consent URL so the caller can print it — the browser may not open (ssh,
/// a headless box), and a login that silently waits on a browser that never appeared is the worst
/// possible failure here.
pub async fn login(
    client_id: &str,
    client_secret: &str,
    scopes: &[&str],
    on_url: impl FnOnce(&str),
) -> Result<Granted> {
    // Port 0 = let the OS pick a free one. Nothing to configure, nothing to collide with.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&scopes.join(" ")),
    );

    // `prompt=consent` is load-bearing: without it Google skips the consent screen for a client
    // you have already approved and returns NO refresh token — the exact silent failure that
    // leaves an account that works until the first hour is up.
    on_url(&url);
    open_browser(&url);

    let code = wait_for_code(listener).await?;

    let response = reqwest::Client::new()
        .post(TOKEN_URL)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(VgoogError::Api { status, message: body });
    }

    let token: TokenResponse = response.json().await?;
    let Some(refresh_token) = token.refresh_token else {
        return Err(VgoogError::Auth(
            "Google returned no refresh token — revoke vgoog at myaccount.google.com/permissions and log in again".into(),
        ));
    };

    Ok(Granted {
        refresh_token,
        access_token: token.access_token,
        expiry: Utc::now() + Duration::seconds(token.expires_in),
    })
}
