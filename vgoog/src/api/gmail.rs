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

/// Build a base64url-encoded RFC 2822 message
pub fn build_raw_email(to: &str, subject: &str, body: &str, cc: Option<&str>, bcc: Option<&str>) -> String {
    let mut msg = format!("To: {to}\r\nSubject: {subject}\r\n");
    if let Some(cc) = cc {
        msg.push_str(&format!("Cc: {cc}\r\n"));
    }
    if let Some(bcc) = bcc {
        msg.push_str(&format!("Bcc: {bcc}\r\n"));
    }
    msg.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\n");
    msg.push_str(body);

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(msg.as_bytes())
}
