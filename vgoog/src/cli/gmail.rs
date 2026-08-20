use crate::api::gmail::GmailApi;
use crate::client::GoogleClient;
use crate::error::{Result, VgoogError};
use serde_json::Value;

fn str_field<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn str_opt<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn u32_field(args: &Value, key: &str, default: u32) -> u32 {
    args.get(key).and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(default)
}

fn str_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = GmailApi::new(client);
    match action {
        "list_messages" => {
            api.list_messages(str_opt(&args, "query"), None, u32_field(&args, "max_results", 20), str_opt(&args, "page_token")).await
        }
        "get_message" => api.get_message(str_field(&args, "id"), str_opt(&args, "format").unwrap_or("full")).await,
        "send_message" => api.send_message(str_field(&args, "raw")).await,
        "trash_message" => api.trash_message(str_field(&args, "id")).await,
        "untrash_message" => api.untrash_message(str_field(&args, "id")).await,
        "delete_message" => api.delete_message(str_field(&args, "id")).await,
        "modify_message" => {
            let add = str_array(&args, "add_labels");
            let remove = str_array(&args, "remove_labels");
            let add_refs: Vec<&str> = add.iter().map(|s| s.as_str()).collect();
            let remove_refs: Vec<&str> = remove.iter().map(|s| s.as_str()).collect();
            api.modify_message(str_field(&args, "id"), &add_refs, &remove_refs).await
        }
        "batch_modify_messages" => {
            let ids = str_array(&args, "ids");
            let add = str_array(&args, "add_labels");
            let remove = str_array(&args, "remove_labels");
            let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            let add_refs: Vec<&str> = add.iter().map(|s| s.as_str()).collect();
            let remove_refs: Vec<&str> = remove.iter().map(|s| s.as_str()).collect();
            api.batch_modify_messages(&id_refs, &add_refs, &remove_refs).await
        }
        "batch_delete_messages" => {
            let ids = str_array(&args, "ids");
            let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            api.batch_delete_messages(&id_refs).await
        }
        "get_attachment" => api.get_attachment(str_field(&args, "message_id"), str_field(&args, "attachment_id")).await,
        "list_threads" => api.list_threads(str_opt(&args, "query"), u32_field(&args, "max_results", 20), str_opt(&args, "page_token")).await,
        "get_thread" => api.get_thread(str_field(&args, "id"), str_opt(&args, "format").unwrap_or("full")).await,
        "trash_thread" => api.trash_thread(str_field(&args, "id")).await,
        "untrash_thread" => api.untrash_thread(str_field(&args, "id")).await,
        "delete_thread" => api.delete_thread(str_field(&args, "id")).await,
        "modify_thread" => {
            let add = str_array(&args, "add_labels");
            let remove = str_array(&args, "remove_labels");
            let add_refs: Vec<&str> = add.iter().map(|s| s.as_str()).collect();
            let remove_refs: Vec<&str> = remove.iter().map(|s| s.as_str()).collect();
            api.modify_thread(str_field(&args, "id"), &add_refs, &remove_refs).await
        }
        "list_labels" => api.list_labels().await,
        "get_label" => api.get_label(str_field(&args, "id")).await,
        "create_label" => api.create_label(
            str_field(&args, "name"),
            str_opt(&args, "label_list_visibility").unwrap_or("labelShow"),
            str_opt(&args, "message_list_visibility").unwrap_or("show"),
        ).await,
        "update_label" => api.update_label(str_field(&args, "id"), str_field(&args, "name")).await,
        "delete_label" => api.delete_label(str_field(&args, "id")).await,
        "list_drafts" => api.list_drafts(u32_field(&args, "max_results", 20), str_opt(&args, "page_token")).await,
        "get_draft" => api.get_draft(str_field(&args, "id"), str_opt(&args, "format").unwrap_or("full")).await,
        "create_draft" => api.create_draft(str_field(&args, "raw")).await,
        "update_draft" => api.update_draft(str_field(&args, "id"), str_field(&args, "raw")).await,
        "send_draft" => api.send_draft(str_field(&args, "id")).await,
        "delete_draft" => api.delete_draft(str_field(&args, "id")).await,
        "get_vacation_settings" => api.get_vacation_settings().await,
        "update_vacation_settings" => api.update_vacation_settings(&args["settings"]).await,
        "get_auto_forwarding" => api.get_auto_forwarding().await,
        "update_auto_forwarding" => api.update_auto_forwarding(&args["settings"]).await,
        "get_imap_settings" => api.get_imap_settings().await,
        "update_imap_settings" => api.update_imap_settings(&args["settings"]).await,
        "get_pop_settings" => api.get_pop_settings().await,
        "update_pop_settings" => api.update_pop_settings(&args["settings"]).await,
        "get_language_settings" => api.get_language_settings().await,
        "update_language_settings" => api.update_language_settings(str_field(&args, "display_language")).await,
        "list_filters" => api.list_filters().await,
        "get_filter" => api.get_filter(str_field(&args, "id")).await,
        "create_filter" => api.create_filter(&args["filter"]).await,
        "delete_filter" => api.delete_filter(str_field(&args, "id")).await,
        "list_forwarding_addresses" => api.list_forwarding_addresses().await,
        "create_forwarding_address" => api.create_forwarding_address(str_field(&args, "email")).await,
        "delete_forwarding_address" => api.delete_forwarding_address(str_field(&args, "email")).await,
        "list_send_as" => api.list_send_as().await,
        "get_send_as" => api.get_send_as(str_field(&args, "email")).await,
        "create_send_as" => api.create_send_as(&args["send_as"]).await,
        "update_send_as" => api.update_send_as(str_field(&args, "email"), &args["send_as"]).await,
        "delete_send_as" => api.delete_send_as(str_field(&args, "email")).await,
        "verify_send_as" => api.verify_send_as(str_field(&args, "email")).await,
        "list_delegates" => api.list_delegates().await,
        "add_delegate" => api.add_delegate(str_field(&args, "email")).await,
        "remove_delegate" => api.remove_delegate(str_field(&args, "email")).await,
        "get_profile" => api.get_profile().await,
        "list_history" => api.list_history(
            str_field(&args, "start_history_id"),
            u32_field(&args, "max_results", 100),
            str_opt(&args, "page_token"),
        ).await,
        _ => Err(VgoogError::Other(format!("Unknown gmail action: {action}"))),
    }
}
