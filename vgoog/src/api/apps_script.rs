use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://script.googleapis.com/v1";

pub struct AppsScriptApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> AppsScriptApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Projects ──

    pub async fn create_project(&self, title: &str, parent_id: Option<&str>) -> Result<Value> {
        let url = format!("{BASE}/projects");
        let mut body = json!({ "title": title });
        if let Some(pid) = parent_id {
            body["parentId"] = json!(pid);
        }
        self.client.post(&url, &body).await
    }

    pub async fn get_project(&self, script_id: &str) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}");
        self.client.get(&url).await
    }

    pub async fn get_content(&self, script_id: &str, version: Option<i64>) -> Result<Value> {
        let mut url = format!("{BASE}/projects/{script_id}/content");
        if let Some(v) = version {
            url.push_str(&format!("?versionNumber={v}"));
        }
        self.client.get(&url).await
    }

    pub async fn update_content(&self, script_id: &str, files: &[Value]) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/content");
        self.client
            .put(&url, &json!({ "scriptId": script_id, "files": files }))
            .await
    }

    pub async fn get_metrics(
        &self,
        script_id: &str,
        filter: Option<&Value>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/projects/{script_id}/metrics");
        if let Some(f) = filter {
            if let Some(deployment_id) = f.get("deploymentId").and_then(|v| v.as_str()) {
                url.push_str(&format!("?metricsFilter.deploymentId={deployment_id}"));
            }
        }
        self.client.get(&url).await
    }

    // ── Versions ──

    pub async fn list_versions(
        &self,
        script_id: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/projects/{script_id}/versions?pageSize={page_size}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn create_version(
        &self,
        script_id: &str,
        description: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/versions");
        self.client
            .post(&url, &json!({ "description": description }))
            .await
    }

    pub async fn get_version(&self, script_id: &str, version_number: i64) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/versions/{version_number}");
        self.client.get(&url).await
    }

    // ── Deployments ──

    pub async fn list_deployments(
        &self,
        script_id: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/projects/{script_id}/deployments?pageSize={page_size}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn create_deployment(
        &self,
        script_id: &str,
        version_number: i64,
        description: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/deployments");
        self.client
            .post(
                &url,
                &json!({
                    "versionNumber": version_number,
                    "manifestFileName": "appsscript",
                    "description": description,
                }),
            )
            .await
    }

    pub async fn get_deployment(
        &self,
        script_id: &str,
        deployment_id: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/deployments/{deployment_id}");
        self.client.get(&url).await
    }

    pub async fn update_deployment(
        &self,
        script_id: &str,
        deployment_id: &str,
        version_number: i64,
        description: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/deployments/{deployment_id}");
        self.client
            .put(
                &url,
                &json!({
                    "deploymentConfig": {
                        "versionNumber": version_number,
                        "manifestFileName": "appsscript",
                        "description": description,
                    }
                }),
            )
            .await
    }

    pub async fn delete_deployment(
        &self,
        script_id: &str,
        deployment_id: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/projects/{script_id}/deployments/{deployment_id}");
        self.client.delete(&url).await
    }

    // ── Execution ──

    pub async fn run(
        &self,
        script_id: &str,
        function_name: &str,
        parameters: Option<&[Value]>,
        dev_mode: bool,
    ) -> Result<Value> {
        let url = format!("{BASE}/scripts/{script_id}:run");
        let mut body = json!({
            "function": function_name,
            "devMode": dev_mode,
        });
        if let Some(params) = parameters {
            body["parameters"] = json!(params);
        }
        self.client.post(&url, &body).await
    }

    // ── Processes ──

    pub async fn list_processes(
        &self,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/processes?pageSize={page_size}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn list_script_processes(
        &self,
        script_id: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/processes:listScriptProcesses?pageSize={page_size}&scriptId={script_id}"
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    // ── Convenience: create script file values ──

    pub fn make_script_file(name: &str, file_type: &str, source: &str) -> Value {
        json!({
            "name": name,
            "type": file_type,
            "source": source,
        })
    }

    pub fn make_server_js_file(name: &str, source: &str) -> Value {
        Self::make_script_file(name, "SERVER_JS", source)
    }

    pub fn make_html_file(name: &str, source: &str) -> Value {
        Self::make_script_file(name, "HTML", source)
    }

    pub fn make_manifest(timezone: &str, dependencies: &Value) -> Value {
        json!({
            "name": "appsscript",
            "type": "JSON",
            "source": serde_json::to_string(&json!({
                "timeZone": timezone,
                "dependencies": dependencies,
                "exceptionLogging": "STACKDRIVER",
            })).unwrap_or_default(),
        })
    }
}
