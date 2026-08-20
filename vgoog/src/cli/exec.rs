use crate::client::GoogleClient;
use crate::error::{Result, VgoogError};
use serde_json::Value;

pub async fn execute(client: &GoogleClient, service: &str, action: &str, args: Value) -> Result<Value> {
    match service {
        "gmail" => super::gmail::execute(client, action, args).await,
        "calendar" => super::calendar::execute(client, action, args).await,
        "drive" => super::drive::execute(client, action, args).await,
        "sheets" => super::sheets::execute(client, action, args).await,
        "docs" => super::docs::execute(client, action, args).await,
        "slides" => super::slides::execute(client, action, args).await,
        "forms" => super::forms::execute(client, action, args).await,
        "tasks" => super::tasks::execute(client, action, args).await,
        "contacts" => super::people::execute(client, action, args).await,
        "apps_script" => super::apps_script::execute(client, action, args).await,
        _ => Err(VgoogError::Other(format!("Unknown service: {service}"))),
    }
}

pub fn list_all() -> Value {
    serde_json::json!({
        "gmail": [
            "list_messages", "get_message", "send_message", "trash_message", "untrash_message",
            "delete_message", "modify_message", "batch_modify_messages", "batch_delete_messages",
            "get_attachment", "list_threads", "get_thread", "trash_thread", "untrash_thread",
            "delete_thread", "modify_thread", "list_labels", "get_label", "create_label",
            "update_label", "delete_label", "list_drafts", "get_draft", "create_draft",
            "update_draft", "send_draft", "delete_draft", "get_vacation_settings",
            "update_vacation_settings", "get_auto_forwarding", "update_auto_forwarding",
            "get_imap_settings", "update_imap_settings", "get_pop_settings", "update_pop_settings",
            "get_language_settings", "update_language_settings", "list_filters", "get_filter",
            "create_filter", "delete_filter", "list_forwarding_addresses",
            "create_forwarding_address", "delete_forwarding_address", "list_send_as",
            "get_send_as", "create_send_as", "update_send_as", "delete_send_as", "verify_send_as",
            "list_delegates", "add_delegate", "remove_delegate", "get_profile", "list_history"
        ],
        "calendar": [
            "list_calendars", "get_calendar", "insert_calendar_to_list", "update_calendar_in_list",
            "remove_calendar_from_list", "create_calendar", "get_calendar_metadata",
            "update_calendar_metadata", "delete_calendar", "clear_calendar", "list_events",
            "get_event", "create_event", "update_event", "delete_event", "move_event",
            "quick_add_event", "list_event_instances", "list_acl", "insert_acl_rule",
            "update_acl_rule", "delete_acl_rule", "list_settings", "get_setting", "get_colors",
            "query_free_busy"
        ],
        "drive": [
            "list_files", "get_file", "create_file", "update_file_metadata", "delete_file",
            "copy_file", "empty_trash", "generate_file_ids", "move_file", "create_folder",
            "list_permissions", "get_permission", "create_permission", "update_permission",
            "delete_permission", "list_comments", "create_comment", "update_comment",
            "delete_comment", "list_replies", "create_reply", "list_revisions", "get_revision",
            "delete_revision", "get_start_page_token", "list_changes", "get_about",
            "list_shared_drives", "create_shared_drive", "delete_shared_drive"
        ],
        "sheets": [
            "create_spreadsheet", "get_spreadsheet", "get_spreadsheet_with_ranges", "get_values",
            "batch_get_values", "update_values", "append_values", "clear_values",
            "batch_update_values", "batch_clear_values", "batch_update", "add_sheet",
            "delete_sheet", "rename_sheet", "auto_resize_columns", "sort_range",
            "create_named_range"
        ],
        "docs": [
            "create_document", "get_document", "batch_update", "insert_text", "delete_content",
            "insert_table", "insert_inline_image", "update_text_style", "update_paragraph_style",
            "replace_all_text", "create_named_range", "insert_page_break", "create_header",
            "create_footer"
        ],
        "slides": [
            "create_presentation", "get_presentation", "get_page", "get_page_thumbnail",
            "batch_update", "create_slide", "delete_slide", "duplicate_slide", "move_slide",
            "insert_text", "delete_text", "replace_all_text", "create_shape", "create_image",
            "create_table", "update_text_style", "update_shape_properties",
            "replace_all_shapes_with_image", "update_page_properties", "create_speaker_notes"
        ],
        "forms": [
            "create_form", "get_form", "batch_update", "list_responses", "get_response",
            "create_watch", "list_watches", "delete_watch", "renew_watch", "add_text_question",
            "add_choice_question", "add_scale_question", "add_date_question", "add_time_question",
            "add_section_header", "delete_item", "move_item", "update_form_info",
            "update_settings", "add_file_upload_question", "add_grid_question"
        ],
        "tasks": [
            "list_task_lists", "get_task_list", "create_task_list", "update_task_list",
            "delete_task_list", "list_tasks", "get_task", "create_task", "update_task",
            "complete_task", "uncomplete_task", "delete_task", "move_task", "clear_completed"
        ],
        "contacts": [
            "get_person", "get_me", "get_batch_people", "list_contacts", "search_contacts",
            "create_contact", "update_contact", "delete_contact", "batch_create_contacts",
            "batch_delete_contacts", "batch_update_contacts", "list_contact_groups",
            "get_contact_group", "create_contact_group", "update_contact_group",
            "delete_contact_group", "modify_contact_group_members", "list_other_contacts",
            "copy_other_contact_to_contacts", "search_directory"
        ],
        "apps_script": [
            "create_project", "get_project", "get_content", "update_content", "get_metrics",
            "list_versions", "create_version", "get_version", "list_deployments",
            "create_deployment", "get_deployment", "update_deployment", "delete_deployment",
            "run", "list_processes", "list_script_processes"
        ]
    })
}
