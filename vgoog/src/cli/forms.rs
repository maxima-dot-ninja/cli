use crate::api::forms::FormsApi;
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

fn b(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn str_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn val_array(args: &Value, key: &str) -> Vec<Value> {
    args.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = FormsApi::new(client);
    match action {
        "create_form" => api.create_form(s(&args, "title"), s(&args, "document_title")).await,
        "get_form" => api.get_form(s(&args, "form_id")).await,
        "batch_update" => {
            let requests = val_array(&args, "requests");
            api.batch_update(s(&args, "form_id"), &requests).await
        }
        "list_responses" => api.list_responses(s(&args, "form_id"), so(&args, "page_token"), u(&args, "page_size", 50)).await,
        "get_response" => api.get_response(s(&args, "form_id"), s(&args, "response_id")).await,
        "create_watch" => api.create_watch(s(&args, "form_id"), s(&args, "event_type"), s(&args, "topic_name")).await,
        "list_watches" => api.list_watches(s(&args, "form_id")).await,
        "delete_watch" => api.delete_watch(s(&args, "form_id"), s(&args, "watch_id")).await,
        "renew_watch" => api.renew_watch(s(&args, "form_id"), s(&args, "watch_id")).await,
        "add_text_question" => api.add_text_question(
            s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
            i(&args, "index", 0), b(&args, "paragraph", false),
        ).await,
        "add_choice_question" => {
            let options = str_array(&args, "options");
            let option_refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            api.add_choice_question(
                s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
                i(&args, "index", 0), s(&args, "choice_type"), &option_refs,
            ).await
        }
        "add_scale_question" => api.add_scale_question(
            s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
            i(&args, "index", 0), i(&args, "low", 1), i(&args, "high", 5),
            s(&args, "low_label"), s(&args, "high_label"),
        ).await,
        "add_date_question" => api.add_date_question(
            s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
            i(&args, "index", 0), b(&args, "include_time", false), b(&args, "include_year", true),
        ).await,
        "add_time_question" => api.add_time_question(
            s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
            i(&args, "index", 0), b(&args, "include_duration", false),
        ).await,
        "add_section_header" => api.add_section_header(
            s(&args, "form_id"), s(&args, "title"), s(&args, "description"), i(&args, "index", 0),
        ).await,
        "delete_item" => api.delete_item(s(&args, "form_id"), i(&args, "index", 0)).await,
        "move_item" => api.move_item(s(&args, "form_id"), i(&args, "original_index", 0), i(&args, "new_index", 1)).await,
        "update_form_info" => api.update_form_info(s(&args, "form_id"), so(&args, "title"), so(&args, "description")).await,
        "update_settings" => api.update_settings(s(&args, "form_id"), &args["settings"], s(&args, "update_mask")).await,
        "add_file_upload_question" => api.add_file_upload_question(
            s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
            i(&args, "index", 0), i(&args, "max_files", 1), so(&args, "max_file_size").unwrap_or("10MB"),
        ).await,
        "add_grid_question" => {
            let rows = str_array(&args, "rows");
            let columns = str_array(&args, "columns");
            let row_refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
            let col_refs: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
            api.add_grid_question(
                s(&args, "form_id"), s(&args, "title"), b(&args, "required", false),
                i(&args, "index", 0), &row_refs, &col_refs,
            ).await
        }
        _ => Err(VgoogError::Other(format!("Unknown forms action: {action}"))),
    }
}
