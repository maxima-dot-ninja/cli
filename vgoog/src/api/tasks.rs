use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://tasks.googleapis.com/tasks/v1";

pub struct TasksApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> TasksApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Task Lists ──

    pub async fn list_task_lists(
        &self,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/users/@me/lists?maxResults={max_results}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_task_list(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/users/@me/lists/{id}");
        self.client.get(&url).await
    }

    pub async fn create_task_list(&self, title: &str) -> Result<Value> {
        let url = format!("{BASE}/users/@me/lists");
        self.client.post(&url, &json!({ "title": title })).await
    }

    pub async fn update_task_list(&self, id: &str, title: &str) -> Result<Value> {
        let url = format!("{BASE}/users/@me/lists/{id}");
        self.client.put(&url, &json!({ "title": title })).await
    }

    pub async fn delete_task_list(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/users/@me/lists/{id}");
        self.client.delete(&url).await
    }

    // ── Tasks ──

    pub async fn list_tasks(
        &self,
        task_list_id: &str,
        max_results: u32,
        page_token: Option<&str>,
        show_completed: bool,
        show_deleted: bool,
        show_hidden: bool,
        due_min: Option<&str>,
        due_max: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/lists/{task_list_id}/tasks?maxResults={max_results}&showCompleted={show_completed}&showDeleted={show_deleted}&showHidden={show_hidden}"
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        if let Some(dm) = due_min {
            url.push_str(&format!("&dueMin={}", urlencoding::encode(dm)));
        }
        if let Some(dm) = due_max {
            url.push_str(&format!("&dueMax={}", urlencoding::encode(dm)));
        }
        self.client.get(&url).await
    }

    pub async fn get_task(&self, task_list_id: &str, task_id: &str) -> Result<Value> {
        let url = format!("{BASE}/lists/{task_list_id}/tasks/{task_id}");
        self.client.get(&url).await
    }

    pub async fn create_task(
        &self,
        task_list_id: &str,
        title: &str,
        notes: Option<&str>,
        due: Option<&str>,
        parent: Option<&str>,
        previous: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/lists/{task_list_id}/tasks?");
        if let Some(p) = parent {
            url.push_str(&format!("parent={p}&"));
        }
        if let Some(pr) = previous {
            url.push_str(&format!("previous={pr}&"));
        }
        let mut body = json!({ "title": title });
        if let Some(n) = notes {
            body["notes"] = json!(n);
        }
        if let Some(d) = due {
            body["due"] = json!(d);
        }
        self.client.post(&url, &body).await
    }

    pub async fn update_task(
        &self,
        task_list_id: &str,
        task_id: &str,
        updates: &Value,
    ) -> Result<Value> {
        let url = format!("{BASE}/lists/{task_list_id}/tasks/{task_id}");
        self.client.patch(&url, updates).await
    }

    pub async fn complete_task(&self, task_list_id: &str, task_id: &str) -> Result<Value> {
        self.update_task(task_list_id, task_id, &json!({ "status": "completed" }))
            .await
    }

    pub async fn uncomplete_task(&self, task_list_id: &str, task_id: &str) -> Result<Value> {
        self.update_task(
            task_list_id,
            task_id,
            &json!({ "status": "needsAction", "completed": null }),
        )
        .await
    }

    pub async fn delete_task(&self, task_list_id: &str, task_id: &str) -> Result<Value> {
        let url = format!("{BASE}/lists/{task_list_id}/tasks/{task_id}");
        self.client.delete(&url).await
    }

    pub async fn move_task(
        &self,
        task_list_id: &str,
        task_id: &str,
        parent: Option<&str>,
        previous: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/lists/{task_list_id}/tasks/{task_id}/move?");
        if let Some(p) = parent {
            url.push_str(&format!("parent={p}&"));
        }
        if let Some(pr) = previous {
            url.push_str(&format!("previous={pr}&"));
        }
        self.client.post_empty(&url).await
    }

    pub async fn clear_completed(&self, task_list_id: &str) -> Result<Value> {
        let url = format!("{BASE}/lists/{task_list_id}/clear");
        self.client.post_empty(&url).await
    }
}
