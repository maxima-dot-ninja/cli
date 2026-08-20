use crate::api::slides::SlidesApi;
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

fn io(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn f(args: &Value, key: &str, default: f64) -> f64 {
    args.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn b(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn val_array(args: &Value, key: &str) -> Vec<Value> {
    args.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = SlidesApi::new(client);
    match action {
        "create_presentation" => api.create_presentation(s(&args, "title")).await,
        "get_presentation" => api.get_presentation(s(&args, "id")).await,
        "get_page" => api.get_page(s(&args, "presentation_id"), s(&args, "page_id")).await,
        "get_page_thumbnail" => api.get_page_thumbnail(s(&args, "presentation_id"), s(&args, "page_id")).await,
        "batch_update" => {
            let requests = val_array(&args, "requests");
            api.batch_update(s(&args, "presentation_id"), &requests).await
        }
        "create_slide" => api.create_slide(s(&args, "presentation_id"), s(&args, "layout"), io(&args, "insertion_index")).await,
        "delete_slide" => api.delete_slide(s(&args, "presentation_id"), s(&args, "slide_id")).await,
        "duplicate_slide" => api.duplicate_slide(s(&args, "presentation_id"), s(&args, "slide_id")).await,
        "move_slide" => api.move_slide(s(&args, "presentation_id"), s(&args, "slide_id"), i(&args, "insertion_index", 0)).await,
        "insert_text" => api.insert_text(
            s(&args, "presentation_id"), s(&args, "object_id"), s(&args, "text"), i(&args, "insertion_index", 0),
        ).await,
        "delete_text" => api.delete_text(
            s(&args, "presentation_id"), s(&args, "object_id"), i(&args, "start_index", 0), i(&args, "end_index", 1),
        ).await,
        "replace_all_text" => api.replace_all_text(
            s(&args, "presentation_id"), s(&args, "find"), s(&args, "replace"), b(&args, "match_case", true),
        ).await,
        "create_shape" => api.create_shape(
            s(&args, "presentation_id"), s(&args, "page_id"), s(&args, "shape_type"),
            f(&args, "x_pt", 100.0), f(&args, "y_pt", 100.0),
            f(&args, "width_pt", 200.0), f(&args, "height_pt", 200.0),
        ).await,
        "create_image" => api.create_image(
            s(&args, "presentation_id"), s(&args, "page_id"), s(&args, "url"),
            f(&args, "x_pt", 100.0), f(&args, "y_pt", 100.0),
            f(&args, "width_pt", 200.0), f(&args, "height_pt", 200.0),
        ).await,
        "create_table" => api.create_table(
            s(&args, "presentation_id"), s(&args, "page_id"),
            i(&args, "rows", 2), i(&args, "cols", 2),
        ).await,
        "update_text_style" => api.update_text_style(
            s(&args, "presentation_id"), s(&args, "object_id"),
            i(&args, "start_index", 0), i(&args, "end_index", 1),
            &args["style"], so(&args, "fields").unwrap_or("*"),
        ).await,
        "update_shape_properties" => api.update_shape_properties(
            s(&args, "presentation_id"), s(&args, "object_id"),
            &args["properties"], so(&args, "fields").unwrap_or("*"),
        ).await,
        "replace_all_shapes_with_image" => api.replace_all_shapes_with_image(
            s(&args, "presentation_id"), s(&args, "find_text"), s(&args, "image_url"), b(&args, "match_case", true),
        ).await,
        "update_page_properties" => api.update_page_properties(
            s(&args, "presentation_id"), s(&args, "page_id"),
            &args["properties"], so(&args, "fields").unwrap_or("*"),
        ).await,
        "create_speaker_notes" => api.create_speaker_notes(
            s(&args, "presentation_id"), s(&args, "slide_id"), s(&args, "notes_text"),
        ).await,
        _ => Err(VgoogError::Other(format!("Unknown slides action: {action}"))),
    }
}
