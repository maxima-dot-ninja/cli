use crate::api::tasks::TasksApi;
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

fn b(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = TasksApi::new(client);
    match action {
        "list_task_lists" => api.list_task_lists(u(&args, "max_results", 20), so(&args, "page_token")).await,
        "get_task_list" => api.get_task_list(s(&args, "id")).await,
        "create_task_list" => api.create_task_list(s(&args, "title")).await,
        "update_task_list" => api.update_task_list(s(&args, "id"), s(&args, "title")).await,
        "delete_task_list" => api.delete_task_list(s(&args, "id")).await,
        "list_tasks" => api.list_tasks(
            s(&args, "task_list_id"), u(&args, "max_results", 20), so(&args, "page_token"),
            b(&args, "show_completed", true), b(&args, "show_deleted", false),
            b(&args, "show_hidden", false), so(&args, "due_min"), so(&args, "due_max"),
        ).await,
        "get_task" => api.get_task(s(&args, "task_list_id"), s(&args, "task_id")).await,
        "create_task" => api.create_task(
            s(&args, "task_list_id"), s(&args, "title"),
            so(&args, "notes"), so(&args, "due"), so(&args, "parent"), so(&args, "previous"),
        ).await,
        "update_task" => api.update_task(s(&args, "task_list_id"), s(&args, "task_id"), &args["updates"]).await,
        "complete_task" => api.complete_task(s(&args, "task_list_id"), s(&args, "task_id")).await,
        "uncomplete_task" => api.uncomplete_task(s(&args, "task_list_id"), s(&args, "task_id")).await,
        "delete_task" => api.delete_task(s(&args, "task_list_id"), s(&args, "task_id")).await,
        "move_task" => api.move_task(
            s(&args, "task_list_id"), s(&args, "task_id"),
            so(&args, "parent"), so(&args, "previous"),
        ).await,
        "clear_completed" => api.clear_completed(s(&args, "task_list_id")).await,
        _ => Err(VgoogError::Other(format!("Unknown tasks action: {action}"))),
    }
}
