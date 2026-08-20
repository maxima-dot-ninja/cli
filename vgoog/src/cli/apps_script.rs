use crate::api::apps_script::AppsScriptApi;
use crate::client::GoogleClient;
use crate::error::{Result, VgoogError};
use serde_json::Value;

fn s<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn so<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn u(args: &Value, key: &str, default: u32) -> u32 {
    args.get(key).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(default)
}

fn i(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

fn io(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn b(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn val_array(args: &Value, key: &str) -> Vec<Value> {
    args.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = AppsScriptApi::new(client);
    match action {
        "create_project" => api.create_project(s(&args, "title"), so(&args, "parent_id")).await,
        "get_project" => api.get_project(s(&args, "script_id")).await,
        "get_content" => api.get_content(s(&args, "script_id"), io(&args, "version")).await,
        "update_content" => {
            let files = val_array(&args, "files");
            api.update_content(s(&args, "script_id"), &files).await
        }
        "get_metrics" => {
            let filter = args.get("filter");
            api.get_metrics(s(&args, "script_id"), filter).await
        }
        "list_versions" => api.list_versions(s(&args, "script_id"), u(&args, "page_size", 20), so(&args, "page_token")).await,
        "create_version" => api.create_version(s(&args, "script_id"), s(&args, "description")).await,
        "get_version" => api.get_version(s(&args, "script_id"), i(&args, "version_number", 1)).await,
        "list_deployments" => api.list_deployments(s(&args, "script_id"), u(&args, "page_size", 20), so(&args, "page_token")).await,
        "create_deployment" => api.create_deployment(
            s(&args, "script_id"), i(&args, "version_number", 1), s(&args, "description"),
        ).await,
        "get_deployment" => api.get_deployment(s(&args, "script_id"), s(&args, "deployment_id")).await,
        "update_deployment" => api.update_deployment(
            s(&args, "script_id"), s(&args, "deployment_id"),
            i(&args, "version_number", 1), s(&args, "description"),
        ).await,
        "delete_deployment" => api.delete_deployment(s(&args, "script_id"), s(&args, "deployment_id")).await,
        "run" => {
            let params = args.get("parameters").and_then(|v| v.as_array());
            let params_ref = params.map(|p| p.as_slice());
            api.run(s(&args, "script_id"), s(&args, "function_name"), params_ref, b(&args, "dev_mode", false)).await
        }
        "list_processes" => api.list_processes(u(&args, "page_size", 20), so(&args, "page_token")).await,
        "list_script_processes" => api.list_script_processes(
            s(&args, "script_id"), u(&args, "page_size", 20), so(&args, "page_token"),
        ).await,
        _ => Err(VgoogError::Other(format!("Unknown apps_script action: {action}"))),
    }
}
