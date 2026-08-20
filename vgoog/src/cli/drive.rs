use crate::api::drive::DriveApi;
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

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = DriveApi::new(client);
    match action {
        "list_files" => api.list_files(
            so(&args, "query"), u(&args, "page_size", 20), so(&args, "page_token"),
            so(&args, "order_by"), so(&args, "fields"), so(&args, "spaces"),
        ).await,
        "get_file" => api.get_file(s(&args, "file_id"), so(&args, "fields")).await,
        "create_file" => api.create_file(&args["metadata"]).await,
        "update_file_metadata" => api.update_file_metadata(s(&args, "file_id"), &args["metadata"]).await,
        "delete_file" => api.delete_file(s(&args, "file_id")).await,
        "copy_file" => api.copy_file(s(&args, "file_id"), &args["metadata"]).await,
        "empty_trash" => api.empty_trash().await,
        "generate_file_ids" => api.generate_file_ids(u(&args, "count", 10)).await,
        "move_file" => api.move_file(s(&args, "file_id"), s(&args, "add_parents"), s(&args, "remove_parents")).await,
        "create_folder" => api.create_folder(s(&args, "name"), so(&args, "parent")).await,
        "list_permissions" => api.list_permissions(s(&args, "file_id")).await,
        "get_permission" => api.get_permission(s(&args, "file_id"), s(&args, "permission_id")).await,
        "create_permission" => api.create_permission(
            s(&args, "file_id"), s(&args, "role"), s(&args, "type"), so(&args, "email"),
        ).await,
        "update_permission" => api.update_permission(s(&args, "file_id"), s(&args, "permission_id"), s(&args, "role")).await,
        "delete_permission" => api.delete_permission(s(&args, "file_id"), s(&args, "permission_id")).await,
        "list_comments" => api.list_comments(s(&args, "file_id"), so(&args, "page_token")).await,
        "create_comment" => api.create_comment(s(&args, "file_id"), s(&args, "content")).await,
        "update_comment" => api.update_comment(s(&args, "file_id"), s(&args, "comment_id"), s(&args, "content")).await,
        "delete_comment" => api.delete_comment(s(&args, "file_id"), s(&args, "comment_id")).await,
        "list_replies" => api.list_replies(s(&args, "file_id"), s(&args, "comment_id")).await,
        "create_reply" => api.create_reply(s(&args, "file_id"), s(&args, "comment_id"), s(&args, "content")).await,
        "list_revisions" => api.list_revisions(s(&args, "file_id")).await,
        "get_revision" => api.get_revision(s(&args, "file_id"), s(&args, "revision_id")).await,
        "delete_revision" => api.delete_revision(s(&args, "file_id"), s(&args, "revision_id")).await,
        "get_start_page_token" => api.get_start_page_token().await,
        "list_changes" => api.list_changes(s(&args, "page_token"), u(&args, "page_size", 100)).await,
        "get_about" => api.get_about().await,
        "list_shared_drives" => api.list_shared_drives(so(&args, "page_token")).await,
        "create_shared_drive" => api.create_shared_drive(s(&args, "name")).await,
        "delete_shared_drive" => api.delete_shared_drive(s(&args, "drive_id")).await,
        _ => Err(VgoogError::Other(format!("Unknown drive action: {action}"))),
    }
}
