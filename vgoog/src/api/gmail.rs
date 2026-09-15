use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

pub struct GmailApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> GmailApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Messages ──

    pub async fn list_messages(
        &self,
        query: Option<&str>,
        label_ids: Option<&[&str]>,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/messages?maxResults={max_results}");
        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(labels) = label_ids {
            for l in labels {
                url.push_str(&format!("&labelIds={l}"));
            }
        }
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_message(&self, id: &str, format: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/{id}?format={format}");
        self.client.get(&url).await
    }

    pub async fn send_message(&self, raw: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/send");
        self.client.post(&url, &json!({ "raw": raw })).await
    }

    pub async fn trash_message(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/{id}/trash");
        self.client.post_empty(&url).await
    }

    pub async fn untrash_message(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/{id}/untrash");
        self.client.post_empty(&url).await
    }

    pub async fn delete_message(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/{id}");
        self.client.delete(&url).await
    }

    pub async fn modify_message(
        &self,
        id: &str,
        add_labels: &[&str],
        remove_labels: &[&str],
    ) -> Result<Value> {
        let url = format!("{BASE}/messages/{id}/modify");
        self.client
            .post(
                &url,
                &json!({
                    "addLabelIds": add_labels,
                    "removeLabelIds": remove_labels,
                }),
            )
            .await
    }

    pub async fn batch_modify_messages(
        &self,
        ids: &[&str],
        add_labels: &[&str],
        remove_labels: &[&str],
    ) -> Result<Value> {
        let url = format!("{BASE}/messages/batchModify");
        self.client
            .post(
                &url,
                &json!({
                    "ids": ids,
                    "addLabelIds": add_labels,
                    "removeLabelIds": remove_labels,
                }),
            )
            .await
    }

    pub async fn batch_delete_messages(&self, ids: &[&str]) -> Result<Value> {
        let url = format!("{BASE}/messages/batchDelete");
        self.client.post(&url, &json!({ "ids": ids })).await
    }

    pub async fn get_attachment(&self, message_id: &str, attachment_id: &str) -> Result<Value> {
        let url = format!("{BASE}/messages/{message_id}/attachments/{attachment_id}");
        self.client.get(&url).await
    }

    // ── Threads ──

    pub async fn list_threads(
        &self,
        query: Option<&str>,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/threads?maxResults={max_results}");
        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_thread(&self, id: &str, format: &str) -> Result<Value> {
        let url = format!("{BASE}/threads/{id}?format={format}");
        self.client.get(&url).await
    }

    pub async fn trash_thread(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/threads/{id}/trash");
        self.client.post_empty(&url).await
    }

    pub async fn untrash_thread(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/threads/{id}/untrash");
        self.client.post_empty(&url).await
    }

    pub async fn delete_thread(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/threads/{id}");
        self.client.delete(&url).await
    }

    pub async fn modify_thread(
        &self,
        id: &str,
        add_labels: &[&str],
        remove_labels: &[&str],
    ) -> Result<Value> {
        let url = format!("{BASE}/threads/{id}/modify");
        self.client
            .post(
                &url,
                &json!({
                    "addLabelIds": add_labels,
                    "removeLabelIds": remove_labels,
                }),
            )
            .await
    }

    // ── Labels ──

    pub async fn list_labels(&self) -> Result<Value> {
        let url = format!("{BASE}/labels");
        self.client.get(&url).await
    }

    pub async fn get_label(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/labels/{id}");
        self.client.get(&url).await
    }

    pub async fn create_label(&self, name: &str, label_list_visibility: &str, message_list_visibility: &str) -> Result<Value> {
        let url = format!("{BASE}/labels");
        self.client
            .post(
                &url,
                &json!({
                    "name": name,
                    "labelListVisibility": label_list_visibility,
                    "messageListVisibility": message_list_visibility,
                }),
            )
            .await
    }

    pub async fn update_label(&self, id: &str, name: &str) -> Result<Value> {
        let url = format!("{BASE}/labels/{id}");
        self.client
            .patch(&url, &json!({ "id": id, "name": name }))
            .await
    }

    pub async fn delete_label(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/labels/{id}");
        self.client.delete(&url).await
    }

    // ── Drafts ──

    pub async fn list_drafts(
        &self,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/drafts?maxResults={max_results}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_draft(&self, id: &str, format: &str) -> Result<Value> {
        let url = format!("{BASE}/drafts/{id}?format={format}");
        self.client.get(&url).await
    }

    pub async fn create_draft(&self, raw: &str) -> Result<Value> {
        let url = format!("{BASE}/drafts");
        self.client
            .post(&url, &json!({ "message": { "raw": raw } }))
            .await
    }

    pub async fn update_draft(&self, id: &str, raw: &str) -> Result<Value> {
        let url = format!("{BASE}/drafts/{id}");
        self.client
            .put(&url, &json!({ "message": { "raw": raw } }))
            .await
    }

    pub async fn send_draft(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/drafts/send");
        self.client.post(&url, &json!({ "id": id })).await
    }

    pub async fn delete_draft(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/drafts/{id}");
        self.client.delete(&url).await
    }

    // ── Settings ──

    pub async fn get_vacation_settings(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/vacation");
        self.client.get(&url).await
    }

    pub async fn update_vacation_settings(&self, settings: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/vacation");
        self.client.put(&url, settings).await
    }

    pub async fn get_auto_forwarding(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/autoForwarding");
        self.client.get(&url).await
    }

    pub async fn update_auto_forwarding(&self, settings: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/autoForwarding");
        self.client.put(&url, settings).await
    }

    pub async fn get_imap_settings(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/imap");
        self.client.get(&url).await
    }

    pub async fn update_imap_settings(&self, settings: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/imap");
        self.client.put(&url, settings).await
    }

    pub async fn get_pop_settings(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/pop");
        self.client.get(&url).await
    }

    pub async fn update_pop_settings(&self, settings: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/pop");
        self.client.put(&url, settings).await
    }

    pub async fn get_language_settings(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/language");
        self.client.get(&url).await
    }

    pub async fn update_language_settings(&self, display_language: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/language");
        self.client
            .put(&url, &json!({ "displayLanguage": display_language }))
            .await
    }

    // ── Filters ──

    pub async fn list_filters(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/filters");
        self.client.get(&url).await
    }

    pub async fn get_filter(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/filters/{id}");
        self.client.get(&url).await
    }

    pub async fn create_filter(&self, filter: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/filters");
        self.client.post(&url, filter).await
    }

    pub async fn delete_filter(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/filters/{id}");
        self.client.delete(&url).await
    }

    // ── Forwarding Addresses ──

    pub async fn list_forwarding_addresses(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/forwardingAddresses");
        self.client.get(&url).await
    }

    pub async fn create_forwarding_address(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/forwardingAddresses");
        self.client
            .post(&url, &json!({ "forwardingEmail": email }))
            .await
    }

    pub async fn delete_forwarding_address(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/forwardingAddresses/{email}");
        self.client.delete(&url).await
    }

    // ── Send As ──

    pub async fn list_send_as(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs");
        self.client.get(&url).await
    }

    pub async fn get_send_as(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs/{email}");
        self.client.get(&url).await
    }

    pub async fn create_send_as(&self, send_as: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs");
        self.client.post(&url, send_as).await
    }

    pub async fn update_send_as(&self, email: &str, send_as: &Value) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs/{email}");
        self.client.patch(&url, send_as).await
    }

    pub async fn delete_send_as(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs/{email}");
        self.client.delete(&url).await
    }

    pub async fn verify_send_as(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/sendAs/{email}/verify");
        self.client.post_empty(&url).await
    }

    // ── Delegates ──

    pub async fn list_delegates(&self) -> Result<Value> {
        let url = format!("{BASE}/settings/delegates");
        self.client.get(&url).await
    }

    pub async fn add_delegate(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/delegates");
        self.client
            .post(&url, &json!({ "delegateEmail": email }))
            .await
    }

    pub async fn remove_delegate(&self, email: &str) -> Result<Value> {
        let url = format!("{BASE}/settings/delegates/{email}");
        self.client.delete(&url).await
    }

    // ── Profile ──

    pub async fn get_profile(&self) -> Result<Value> {
        let url = format!("{BASE}/profile");
        self.client.get(&url).await
    }

    // ── History ──

    pub async fn list_history(
        &self,
        start_history_id: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/history?startHistoryId={start_history_id}&maxResults={max_results}"
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }
}

/// Build a base64url-encoded RFC 2822 message. The body goes out as multipart/alternative: the text
/// as written, and an HTML copy where a blank line starts a new `<p>` and a single newline is a
/// `<br>`. Sent as text/plain alone, the line breaks were left to each client's reflow.
pub fn build_raw_email(to: &str, subject: &str, body: &str, cc: Option<&str>, bcc: Option<&str>) -> String {
    let mut msg = format!("To: {to}\r\nSubject: {}\r\n", encode_header(subject));
    if let Some(cc) = cc {
        msg.push_str(&format!("Cc: {cc}\r\n"));
    }
    if let Some(bcc) = bcc {
        msg.push_str(&format!("Bcc: {bcc}\r\n"));
    }
    let body = normalize_newlines(body);
    msg.push_str(&format!("MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"{BOUNDARY}\"\r\n\r\n"));
    msg.push_str(&mime_part("text/plain", &body.replace('\n', "\r\n")));
    msg.push_str(&mime_part("text/html", &body_html(&body)));
    msg.push_str(&format!("--{BOUNDARY}--\r\n"));

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(msg.as_bytes())
}

/// Fixed, so the same input always builds the same message. Both parts are base64, whose alphabet has
/// no `_`, so no line of content can match it.
const BOUNDARY: &str = "vgoog_alt_boundary";

/// One part of the alternative, base64 wrapped at 76 columns. Base64 carries every byte and every line
/// break through untouched, UTF-8 and over-long lines included.
fn mime_part(content_type: &str, content: &str) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let encoded = STANDARD.encode(content);
    let lines: Vec<_> = encoded.as_bytes().chunks(76).map(String::from_utf8_lossy).collect();
    format!(
        "--{BOUNDARY}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
        lines.join("\r\n")
    )
}

/// `\r\n` and a lone `\r` both become `\n`, so a body from any platform splits the same way.
fn normalize_newlines(body: &str) -> String {
    body.replace("\r\n", "\n").replace('\r', "\n")
}

/// Each `\n\n` closes a paragraph and each remaining `\n` is a `<br>`, so every newline in the body
/// shows up in the HTML.
fn body_html(body: &str) -> String {
    body.split("\n\n").map(paragraph_html).collect()
}

/// An empty paragraph (four newlines in a row, or a body ending in a blank line) holds a `<br>`, so
/// the gap renders instead of collapsing into the paragraph margin.
fn paragraph_html(text: &str) -> String {
    if text.is_empty() {
        return "<p><br></p>".to_string();
    }
    format!("<p>{}</p>", escape_html(text).replace('\n', "<br>"))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Header values are 7-bit. A UTF-8 subject written as-is reaches Gmail as mojibake (`✓` became
/// `âœ“`), so anything non-ASCII goes out as an RFC 2047 encoded word.
fn encode_header(value: &str) -> String {
    if value.is_ascii() {
        return value.to_string();
    }
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    format!("=?UTF-8?B?{}?=", STANDARD.encode(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_ascii_subject_is_encoded() {
        assert_eq!(encode_header("Hi"), "Hi");
        assert_eq!(encode_header("✓"), "=?UTF-8?B?4pyT?=");
    }

    fn decode(raw: &str) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        String::from_utf8(URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap()
    }

    #[test]
    fn message_is_multipart_alternative_with_text_and_html() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        let msg = decode(&build_raw_email("a@b.c", "Hi", "one\ntwo\n\nthree", None, None));
        let part = |kind: &str, content: &str| {
            format!("Content-Type: {kind}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n", STANDARD.encode(content))
        };
        assert!(msg.contains("MIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"vgoog_alt_boundary\"\r\n"));
        assert!(msg.contains(&part("text/plain", "one\r\ntwo\r\n\r\nthree")), "{msg}");
        assert!(msg.contains(&part("text/html", "<p>one<br>two</p><p>three</p>")), "{msg}");
        assert!(msg.ends_with("\r\n--vgoog_alt_boundary--\r\n"));
    }

    #[test]
    fn every_newline_style_builds_the_same_message() {
        let unix = build_raw_email("a@b.c", "Hi", "a\nb\n\nc", None, None);
        assert_eq!(build_raw_email("a@b.c", "Hi", "a\r\nb\r\n\r\nc", None, None), unix);
        assert_eq!(build_raw_email("a@b.c", "Hi", "a\rb\r\rc", None, None), unix);
    }

    #[test]
    fn every_newline_reaches_the_html() {
        assert_eq!(body_html("a\nb\n\nc"), "<p>a<br>b</p><p>c</p>");
        assert_eq!(body_html("a\n\n\nb"), "<p>a</p><p><br>b</p>");
        assert_eq!(body_html("a\n\n\n\nb"), "<p>a</p><p><br></p><p>b</p>");
        assert_eq!(body_html("a\n"), "<p>a<br></p>");
    }

    #[test]
    fn body_text_is_escaped_not_rendered() {
        assert_eq!(body_html("1 < 2 & \"<b>\""), "<p>1 &lt; 2 &amp; &quot;&lt;b&gt;&quot;</p>");
    }

    #[test]
    fn long_bodies_wrap_at_76_columns() {
        let msg = decode(&build_raw_email("a@b.c", "Hi", &"x".repeat(500), None, None));
        assert!(msg.lines().all(|l| l.len() <= 76), "{msg}");
    }
}
