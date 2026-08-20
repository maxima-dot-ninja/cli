use crate::api::docs::DocsApi;
use crate::client::GoogleClient;
use crate::error::{Result, VgoogError};
use serde_json::Value;

fn s<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn so<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn i(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

fn f(args: &Value, key: &str, default: f64) -> f64 {
    args.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn bo(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn fo(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

fn io(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn val_array(args: &Value, key: &str) -> Vec<Value> {
    args.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = DocsApi::new(client);
    match action {
        "create_document" => api.create_document(s(&args, "title")).await,
        "get_document" => api.get_document(s(&args, "document_id")).await,
        "batch_update" => {
            let requests = val_array(&args, "requests");
            api.batch_update(s(&args, "document_id"), &requests).await
        }
        "insert_text" => api.insert_text(s(&args, "document_id"), s(&args, "text"), i(&args, "index", 1)).await,
        "delete_content" => api.delete_content(s(&args, "document_id"), i(&args, "start_index", 1), i(&args, "end_index", 2)).await,
        "insert_table" => api.insert_table(s(&args, "document_id"), i(&args, "rows", 2), i(&args, "cols", 2), i(&args, "index", 1)).await,
        "insert_inline_image" => api.insert_inline_image(
            s(&args, "document_id"), s(&args, "uri"), i(&args, "index", 1),
            f(&args, "width_pt", 200.0), f(&args, "height_pt", 200.0),
        ).await,
        "update_text_style" => api.update_text_style(
            s(&args, "document_id"), i(&args, "start_index", 1), i(&args, "end_index", 2),
            bo(&args, "bold"), bo(&args, "italic"), bo(&args, "underline"), fo(&args, "font_size"),
        ).await,
        "update_paragraph_style" => api.update_paragraph_style(
            s(&args, "document_id"), i(&args, "start_index", 1), i(&args, "end_index", 2),
            so(&args, "named_style").unwrap_or("NORMAL_TEXT"),
        ).await,
        "replace_all_text" => api.replace_all_text(
            s(&args, "document_id"), s(&args, "find"), s(&args, "replace"),
            args.get("match_case").and_then(|v| v.as_bool()).unwrap_or(true),
        ).await,
        "create_named_range" => api.create_named_range(
            s(&args, "document_id"), s(&args, "name"),
            i(&args, "start_index", 1), i(&args, "end_index", 2),
        ).await,
        "insert_page_break" => api.insert_page_break(s(&args, "document_id"), i(&args, "index", 1)).await,
        "create_header" => api.create_header(s(&args, "document_id"), io(&args, "section_idx")).await,
        "create_footer" => api.create_footer(s(&args, "document_id"), io(&args, "section_idx")).await,
        _ => Err(VgoogError::Other(format!("Unknown docs action: {action}"))),
    }
}
