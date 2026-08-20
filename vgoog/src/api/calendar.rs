use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://www.googleapis.com/calendar/v3";

pub struct CalendarApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> CalendarApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Calendar List ──

    pub async fn list_calendars(&self, page_token: Option<&str>) -> Result<Value> {
        let mut url = format!("{BASE}/users/me/calendarList");
        if let Some(pt) = page_token {
            url.push_str(&format!("?pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_calendar(&self, id: &str) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/users/me/calendarList/{id}");
        self.client.get(&url).await
    }

    pub async fn insert_calendar_to_list(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/users/me/calendarList");
        self.client.post(&url, &json!({ "id": id })).await
    }

    pub async fn update_calendar_in_list(&self, id: &str, updates: &Value) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/users/me/calendarList/{id}");
        self.client.patch(&url, updates).await
    }

    pub async fn remove_calendar_from_list(&self, id: &str) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/users/me/calendarList/{id}");
        self.client.delete(&url).await
    }

    // ── Calendars ──

    pub async fn create_calendar(&self, summary: &str) -> Result<Value> {
        let url = format!("{BASE}/calendars");
        self.client
            .post(&url, &json!({ "summary": summary }))
            .await
    }

    pub async fn get_calendar_metadata(&self, id: &str) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/calendars/{id}");
        self.client.get(&url).await
    }

    pub async fn update_calendar_metadata(&self, id: &str, updates: &Value) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/calendars/{id}");
        self.client.patch(&url, updates).await
    }

    pub async fn delete_calendar(&self, id: &str) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/calendars/{id}");
        self.client.delete(&url).await
    }

    pub async fn clear_calendar(&self, id: &str) -> Result<Value> {
        let id = urlencoding::encode(id);
        let url = format!("{BASE}/calendars/{id}/clear");
        self.client.post_empty(&url).await
    }

    // ── Events ──

    pub async fn list_events(
        &self,
        calendar_id: &str,
        time_min: Option<&str>,
        time_max: Option<&str>,
        query: Option<&str>,
        max_results: u32,
        page_token: Option<&str>,
        single_events: bool,
        order_by: Option<&str>,
    ) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let mut url = format!(
            "{BASE}/calendars/{cal}/events?maxResults={max_results}&singleEvents={single_events}"
        );
        if let Some(t) = time_min {
            url.push_str(&format!("&timeMin={}", urlencoding::encode(t)));
        }
        if let Some(t) = time_max {
            url.push_str(&format!("&timeMax={}", urlencoding::encode(t)));
        }
        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        if let Some(ob) = order_by {
            url.push_str(&format!("&orderBy={ob}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_event(&self, calendar_id: &str, event_id: &str) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/events/{event_id}");
        self.client.get(&url).await
    }

    pub async fn create_event(&self, calendar_id: &str, event: &Value) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/events");
        self.client.post(&url, event).await
    }

    pub async fn update_event(
        &self,
        calendar_id: &str,
        event_id: &str,
        event: &Value,
    ) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/events/{event_id}");
        self.client.patch(&url, event).await
    }

    pub async fn delete_event(&self, calendar_id: &str, event_id: &str) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/events/{event_id}");
        self.client.delete(&url).await
    }

    pub async fn move_event(
        &self,
        calendar_id: &str,
        event_id: &str,
        destination: &str,
    ) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let dest = urlencoding::encode(destination);
        let url = format!("{BASE}/calendars/{cal}/events/{event_id}/move?destination={dest}");
        self.client.post_empty(&url).await
    }

    pub async fn quick_add_event(&self, calendar_id: &str, text: &str) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!(
            "{BASE}/calendars/{cal}/events/quickAdd?text={}",
            urlencoding::encode(text)
        );
        self.client.post_empty(&url).await
    }

    pub async fn list_event_instances(
        &self,
        calendar_id: &str,
        event_id: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let mut url = format!(
            "{BASE}/calendars/{cal}/events/{event_id}/instances?maxResults={max_results}"
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    // ── ACL ──

    pub async fn list_acl(&self, calendar_id: &str) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/acl");
        self.client.get(&url).await
    }

    pub async fn insert_acl_rule(&self, calendar_id: &str, rule: &Value) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/acl");
        self.client.post(&url, rule).await
    }

    pub async fn update_acl_rule(
        &self,
        calendar_id: &str,
        rule_id: &str,
        rule: &Value,
    ) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/acl/{rule_id}");
        self.client.put(&url, rule).await
    }

    pub async fn delete_acl_rule(&self, calendar_id: &str, rule_id: &str) -> Result<Value> {
        let cal = urlencoding::encode(calendar_id);
        let url = format!("{BASE}/calendars/{cal}/acl/{rule_id}");
        self.client.delete(&url).await
    }

    // ── Settings ──

    pub async fn list_settings(&self) -> Result<Value> {
        let url = format!("{BASE}/users/me/settings");
        self.client.get(&url).await
    }

    pub async fn get_setting(&self, setting: &str) -> Result<Value> {
        let url = format!("{BASE}/users/me/settings/{setting}");
        self.client.get(&url).await
    }

    // ── Colors ──

    pub async fn get_colors(&self) -> Result<Value> {
        let url = format!("{BASE}/colors");
        self.client.get(&url).await
    }

    // ── Free/Busy ──

    pub async fn query_free_busy(&self, body: &Value) -> Result<Value> {
        let url = format!("{BASE}/freeBusy");
        self.client.post(&url, body).await
    }
}
