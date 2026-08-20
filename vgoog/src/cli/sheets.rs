use crate::api::sheets::SheetsApi;
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
    args.get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = SheetsApi::new(client);
    match action {
        "create_spreadsheet" => api.create_spreadsheet(s(&args, "title")).await,
        "get_spreadsheet" => api.get_spreadsheet(s(&args, "id")).await,
        "get_spreadsheet_with_ranges" => {
            let ranges = str_array(&args, "ranges");
            let range_refs: Vec<&str> = ranges.iter().map(|s| s.as_str()).collect();
            api.get_spreadsheet_with_ranges(s(&args, "id"), &range_refs).await
        }
        "get_values" => api.get_values(s(&args, "spreadsheet_id"), s(&args, "range"), so(&args, "value_render")).await,
        "batch_get_values" => {
            let ranges = str_array(&args, "ranges");
            let range_refs: Vec<&str> = ranges.iter().map(|s| s.as_str()).collect();
            api.batch_get_values(s(&args, "spreadsheet_id"), &range_refs).await
        }
        "update_values" => api.update_values(
            s(&args, "spreadsheet_id"), s(&args, "range"), &args["values"],
            so(&args, "input_option").unwrap_or("USER_ENTERED"),
        ).await,
        "append_values" => api.append_values(
            s(&args, "spreadsheet_id"), s(&args, "range"), &args["values"],
            so(&args, "input_option").unwrap_or("USER_ENTERED"),
        ).await,
        "clear_values" => api.clear_values(s(&args, "spreadsheet_id"), s(&args, "range")).await,
        "batch_update_values" => {
            let data = val_array(&args, "data");
            api.batch_update_values(
                s(&args, "spreadsheet_id"), &data,
                so(&args, "input_option").unwrap_or("USER_ENTERED"),
            ).await
        }
        "batch_clear_values" => {
            let ranges = str_array(&args, "ranges");
            let range_refs: Vec<&str> = ranges.iter().map(|s| s.as_str()).collect();
            api.batch_clear_values(s(&args, "spreadsheet_id"), &range_refs).await
        }
        "batch_update" => {
            let requests = val_array(&args, "requests");
            api.batch_update(s(&args, "spreadsheet_id"), &requests).await
        }
        "add_sheet" => api.add_sheet(s(&args, "spreadsheet_id"), s(&args, "title")).await,
        "delete_sheet" => api.delete_sheet(s(&args, "spreadsheet_id"), i(&args, "sheet_id", 0)).await,
        "rename_sheet" => api.rename_sheet(s(&args, "spreadsheet_id"), i(&args, "sheet_id", 0), s(&args, "new_title")).await,
        "auto_resize_columns" => api.auto_resize_columns(
            s(&args, "spreadsheet_id"), i(&args, "sheet_id", 0),
            i(&args, "start_col", 0), i(&args, "end_col", 26),
        ).await,
        "sort_range" => api.sort_range(
            s(&args, "spreadsheet_id"), i(&args, "sheet_id", 0),
            i(&args, "start_row", 0), i(&args, "end_row", 100),
            i(&args, "start_col", 0), i(&args, "end_col", 26),
            i(&args, "sort_col", 0), b(&args, "ascending", true),
        ).await,
        "create_named_range" => api.create_named_range(
            s(&args, "spreadsheet_id"), s(&args, "name"), i(&args, "sheet_id", 0),
            i(&args, "start_row", 0), i(&args, "end_row", 100),
            i(&args, "start_col", 0), i(&args, "end_col", 26),
        ).await,
        _ => Err(VgoogError::Other(format!("Unknown sheets action: {action}"))),
    }
}
