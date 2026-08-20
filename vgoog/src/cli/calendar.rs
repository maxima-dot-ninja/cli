use crate::api::calendar::CalendarApi;
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
    let api = CalendarApi::new(client);
    match action {
        "list_calendars" => api.list_calendars(so(&args, "page_token")).await,
        "get_calendar" => api.get_calendar(s(&args, "id")).await,
        "insert_calendar_to_list" => api.insert_calendar_to_list(s(&args, "id")).await,
        "update_calendar_in_list" => api.update_calendar_in_list(s(&args, "id"), &args["updates"]).await,
        "remove_calendar_from_list" => api.remove_calendar_from_list(s(&args, "id")).await,
        "create_calendar" => api.create_calendar(s(&args, "summary")).await,
        "get_calendar_metadata" => api.get_calendar_metadata(s(&args, "id")).await,
        "update_calendar_metadata" => api.update_calendar_metadata(s(&args, "id"), &args["updates"]).await,
        "delete_calendar" => api.delete_calendar(s(&args, "id")).await,
        "clear_calendar" => api.clear_calendar(s(&args, "id")).await,
        "list_events" => api.list_events(
            so(&args, "calendar_id").unwrap_or("primary"),
            so(&args, "time_min"), so(&args, "time_max"), so(&args, "query"),
            u(&args, "max_results", 20), so(&args, "page_token"),
            b(&args, "single_events", false), so(&args, "order_by"),
        ).await,
        "get_event" => api.get_event(s(&args, "calendar_id"), s(&args, "event_id")).await,
        "create_event" => api.create_event(so(&args, "calendar_id").unwrap_or("primary"), &args["event"]).await,
        "update_event" => api.update_event(s(&args, "calendar_id"), s(&args, "event_id"), &args["event"]).await,
        "delete_event" => api.delete_event(s(&args, "calendar_id"), s(&args, "event_id")).await,
        "move_event" => api.move_event(s(&args, "calendar_id"), s(&args, "event_id"), s(&args, "destination")).await,
        "quick_add_event" => api.quick_add_event(so(&args, "calendar_id").unwrap_or("primary"), s(&args, "text")).await,
        "list_event_instances" => api.list_event_instances(
            s(&args, "calendar_id"), s(&args, "event_id"),
            u(&args, "max_results", 20), so(&args, "page_token"),
        ).await,
        "list_acl" => api.list_acl(s(&args, "calendar_id")).await,
        "insert_acl_rule" => api.insert_acl_rule(s(&args, "calendar_id"), &args["rule"]).await,
        "update_acl_rule" => api.update_acl_rule(s(&args, "calendar_id"), s(&args, "rule_id"), &args["rule"]).await,
        "delete_acl_rule" => api.delete_acl_rule(s(&args, "calendar_id"), s(&args, "rule_id")).await,
        "list_settings" => api.list_settings().await,
        "get_setting" => api.get_setting(s(&args, "setting")).await,
        "get_colors" => api.get_colors().await,
        "query_free_busy" => api.query_free_busy(&args["body"]).await,
        _ => Err(VgoogError::Other(format!("Unknown calendar action: {action}"))),
    }
}
