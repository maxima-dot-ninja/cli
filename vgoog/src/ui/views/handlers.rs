use crate::api::gmail::{self, GmailApi};
use crate::api::calendar::CalendarApi;
use crate::api::drive::DriveApi;
use crate::api::sheets::SheetsApi;
use crate::api::docs::DocsApi;
use crate::api::slides::SlidesApi;
use crate::api::forms::FormsApi;
use crate::api::tasks::TasksApi;
use crate::api::people::PeopleApi;
use crate::api::apps_script::AppsScriptApi;
use crate::ui::app::{App, InputField, ListItem, Screen, Service};
use serde_json::Value;

pub async fn execute_action(app: &mut App) {
    let service = app.current_service();
    let action_idx = app.selected_action;
    app.loading = true;

    let result = match service {
        Service::Gmail => handle_gmail(app, action_idx).await,
        Service::Calendar => handle_calendar(app, action_idx).await,
        Service::Drive => handle_drive(app, action_idx).await,
        Service::Sheets => handle_sheets(app, action_idx).await,
        Service::Docs => handle_docs(app, action_idx).await,
        Service::Slides => handle_slides(app, action_idx).await,
        Service::Forms => handle_forms(app, action_idx).await,
        Service::Tasks => handle_tasks(app, action_idx).await,
        Service::People => handle_people(app, action_idx).await,
        Service::AppsScript => handle_apps_script(app, action_idx).await,
    };

    app.loading = false;
    if let Err(e) = result {
        app.set_status(format!("Error: {e}"));
    }
}

pub async fn execute_detail(app: &mut App) {
    if let Some(item) = app.current_item().cloned() {
        let service = app.service.unwrap_or(app.current_service());
        app.loading = true;

        let result = match service {
            Service::Gmail => {
                let api = GmailApi::new(&app.client);
                api.get_message(&item.id, "full").await
            }
            Service::Calendar => {
                let cal_id = item.metadata.get("calendarId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("primary");
                let api = CalendarApi::new(&app.client);
                api.get_event(cal_id, &item.id).await
            }
            Service::Drive => {
                let api = DriveApi::new(&app.client);
                api.get_file(&item.id, None).await
            }
            Service::Sheets => {
                let api = SheetsApi::new(&app.client);
                api.get_spreadsheet(&item.id).await
            }
            Service::Docs => {
                let api = DocsApi::new(&app.client);
                api.get_document(&item.id).await
            }
            Service::Slides => {
                let api = SlidesApi::new(&app.client);
                api.get_presentation(&item.id).await
            }
            Service::Forms => {
                let api = FormsApi::new(&app.client);
                api.get_form(&item.id).await
            }
            Service::Tasks => {
                let list_id = item.metadata.get("taskListId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let api = TasksApi::new(&app.client);
                api.get_task(list_id, &item.id).await
            }
            Service::People => {
                let api = PeopleApi::new(&app.client);
                api.get_person(&item.id, "names,emailAddresses,phoneNumbers,organizations,addresses,biographies,birthdays,urls").await
            }
            Service::AppsScript => {
                let api = AppsScriptApi::new(&app.client);
                api.get_project(&item.id).await
            }
        };

        app.loading = false;
        match result {
            Ok(val) => {
                app.detail = Some(val);
                app.scroll_offset = 0;
                app.set_status("Detail loaded. ↑↓ to scroll, Esc to go back.");
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        }
    }
}

pub async fn execute_delete(app: &mut App) {
    if let Some(item) = app.current_item().cloned() {
        let service = app.service.unwrap_or(app.current_service());
        app.loading = true;

        let result = match service {
            Service::Gmail => {
                let api = GmailApi::new(&app.client);
                api.trash_message(&item.id).await
            }
            Service::Calendar => {
                let cal_id = item.metadata.get("calendarId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("primary");
                let api = CalendarApi::new(&app.client);
                api.delete_event(cal_id, &item.id).await
            }
            Service::Drive => {
                let api = DriveApi::new(&app.client);
                api.delete_file(&item.id).await
            }
            Service::Tasks => {
                let list_id = item.metadata.get("taskListId")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let api = TasksApi::new(&app.client);
                api.delete_task(list_id, &item.id).await
            }
            Service::People => {
                let api = PeopleApi::new(&app.client);
                api.delete_contact(&item.id).await
            }
            _ => {
                app.loading = false;
                app.set_status("Delete not supported for this service");
                return;
            }
        };

        app.loading = false;
        match result {
            Ok(_) => {
                app.items.retain(|i| i.id != item.id);
                if app.item_cursor > 0 && app.item_cursor >= app.items.len() {
                    app.item_cursor = app.items.len().saturating_sub(1);
                }
                app.set_status("Deleted successfully");
            }
            Err(e) => app.set_status(format!("Error: {e}")),
        }
    }
}

pub async fn submit_input(app: &mut App) {
    let service = app.service.unwrap_or(app.current_service());
    let fields: Vec<InputField> = app.input_fields.clone();
    app.loading = true;

    let result = match service {
        Service::Gmail => submit_gmail(app, &fields).await,
        Service::Calendar => submit_calendar(app, &fields).await,
        Service::Drive => submit_drive(app, &fields).await,
        Service::Sheets => submit_sheets(app, &fields).await,
        Service::Docs => submit_docs(app, &fields).await,
        Service::Slides => submit_slides(app, &fields).await,
        Service::Forms => submit_forms(app, &fields).await,
        Service::Tasks => submit_tasks(app, &fields).await,
        Service::People => submit_people(app, &fields).await,
        Service::AppsScript => submit_apps_script(app, &fields).await,
    };

    app.loading = false;
    match result {
        Ok(msg) => {
            app.set_status(msg);
            app.screen = Screen::ActionSelect;
            app.input_fields.clear();
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

pub async fn load_next_page(app: &mut App) {
    if let Some(token) = app.next_page_token.clone() {
        let service = app.service.unwrap_or(app.current_service());
        app.loading = true;
        app.set_status("Loading next page...");

        // Re-execute the same action with the page token
        let _ = match service {
            Service::Gmail => {
                let api = GmailApi::new(&app.client);
                match api.list_messages(None, None, 20, Some(&token)).await {
                    Ok(val) => {
                        parse_gmail_messages(app, &val);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Service::Drive => {
                let api = DriveApi::new(&app.client);
                match api.list_files(None, 20, Some(&token), None, None, None).await {
                    Ok(val) => {
                        parse_drive_files(app, &val);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            _ => Ok(()),
        };
        app.loading = false;
    }
}

// ── Gmail handlers ──

async fn handle_gmail(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = GmailApi::new(&app.client);
    match action {
        0 => {
            // Inbox
            let val = api.list_messages(Some("in:inbox"), None, 20, None).await?;
            parse_gmail_messages(app, &val);
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} messages loaded", app.items.len()));
        }
        1 => {
            // Search
            app.input_fields = vec![InputField::new("Search Query", "from:someone@gmail.com", true)];
            app.input_field_cursor = 0;
            app.service = Some(Service::Gmail);
            app.screen = Screen::Input;
        }
        2 => {
            // Compose
            app.input_fields = vec![
                InputField::new("To", "recipient@example.com", true),
                InputField::new("Subject", "Email subject", true),
                InputField::new("CC", "cc@example.com", false),
                InputField::new("BCC", "bcc@example.com", false),
                InputField::new("Body", "Type your message...", true).multiline(),
            ];
            app.input_field_cursor = 0;
            app.service = Some(Service::Gmail);
            app.screen = Screen::Input;
        }
        3 => {
            // Labels
            let val = api.list_labels().await?;
            let labels = val.get("labels").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                labels
                    .iter()
                    .map(|l| ListItem {
                        id: l.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: l.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed").to_string(),
                        subtitle: l.get("type").and_then(|v| v.as_str()).unwrap_or("user").to_string(),
                        metadata: l.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} labels", app.items.len()));
        }
        4 => {
            // Drafts
            let val = api.list_drafts(20, None).await?;
            let drafts = val.get("drafts").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                drafts
                    .iter()
                    .map(|d| ListItem {
                        id: d.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: format!("Draft {}", d.get("id").and_then(|v| v.as_str()).unwrap_or("")),
                        subtitle: "Draft message".to_string(),
                        metadata: d.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} drafts", app.items.len()));
        }
        5 => {
            // Threads
            let val = api.list_threads(None, 20, None).await?;
            let threads = val.get("threads").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                threads
                    .iter()
                    .map(|t| ListItem {
                        id: t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: format!("Thread {}", t.get("id").and_then(|v| v.as_str()).unwrap_or("")),
                        subtitle: t.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        metadata: t.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} threads", app.items.len()));
        }
        6 => {
            // Filters
            let val = api.list_filters().await?;
            let filters = val.get("filter").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                filters
                    .iter()
                    .map(|f| ListItem {
                        id: f.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: format!("Filter {}", f.get("id").and_then(|v| v.as_str()).unwrap_or("")),
                        subtitle: "Gmail filter".to_string(),
                        metadata: f.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} filters", app.items.len()));
        }
        7 => {
            // Settings (vacation)
            let val = api.get_vacation_settings().await?;
            app.detail = Some(val);
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
            app.set_status("Vacation settings loaded");
        }
        8 => {
            // Forwarding
            let val = api.list_forwarding_addresses().await?;
            let addrs = val.get("forwardingAddresses").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                addrs
                    .iter()
                    .map(|a| ListItem {
                        id: a.get("forwardingEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: a.get("forwardingEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        subtitle: a.get("verificationStatus").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        metadata: a.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
        }
        9 => {
            // Send-As
            let val = api.list_send_as().await?;
            let aliases = val.get("sendAs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                aliases
                    .iter()
                    .map(|a| ListItem {
                        id: a.get("sendAsEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: a.get("sendAsEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        subtitle: a.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        metadata: a.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
        }
        10 => {
            // Delegates
            let val = api.list_delegates().await?;
            let delegates = val.get("delegates").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                delegates
                    .iter()
                    .map(|d| ListItem {
                        id: d.get("delegateEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: d.get("delegateEmail").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        subtitle: d.get("verificationStatus").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        metadata: d.clone(),
                    })
                    .collect(),
            );
            app.service = Some(Service::Gmail);
            app.screen = Screen::ActionView;
        }
        11 => {
            // Unified Search
            app.input_fields = vec![InputField::new("Search Query (all accounts)", "from:someone@gmail.com", true)];
            app.input_field_cursor = 0;
            app.service = Some(Service::Gmail);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

fn parse_gmail_messages(app: &mut App, val: &Value) {
    app.next_page_token = val
        .get("nextPageToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let messages = val.get("messages").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    app.set_items(
        messages
            .iter()
            .map(|m| {
                let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let snippet = m.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let _thread_id = m.get("threadId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                ListItem {
                    id: id.clone(),
                    title: format!("Message {}", &id[..id.len().min(12)]),
                    subtitle: if snippet.len() > 80 {
                        format!("{}...", &snippet[..77])
                    } else {
                        snippet
                    },
                    metadata: m.clone(),
                }
            })
            .collect(),
    );
    app.page_info = if app.next_page_token.is_some() {
        "more available".to_string()
    } else {
        String::new()
    };
}

async fn submit_gmail(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = GmailApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            // Search
            let query = &fields[0].value;
            let val = api.list_messages(Some(query), None, 20, None).await?;
            parse_gmail_messages(app, &val);
            app.screen = Screen::ActionView;
            Ok(format!("{} messages found", app.items.len()))
        }
        2 => {
            // Compose
            let to = &fields[0].value;
            let subject = &fields[1].value;
            let cc = if fields[2].value.is_empty() { None } else { Some(fields[2].value.as_str()) };
            let bcc = if fields[3].value.is_empty() { None } else { Some(fields[3].value.as_str()) };
            let body = &fields[4].value;
            let raw = gmail::build_raw_email(to, subject, body, cc, bcc);
            api.send_message(&raw).await?;
            Ok("Email sent successfully!".to_string())
        }
        11 => {
            // Unified Search — search across all accounts
            let query = fields[0].value.clone();
            let original_account = app.client.active_account_name().await;
            let all_accounts = app.client.account_names().await;
            let mut all_items: Vec<ListItem> = Vec::new();
            let mut errors: Vec<String> = Vec::new();

            for account_name in &all_accounts {
                let label = if *account_name == original_account {
                    // Already on this account, no need to switch
                    app.client.active_account_label().await
                } else {
                    match app.client.switch_account(account_name).await {
                        Ok(()) => app.client.active_account_label().await,
                        Err(e) => {
                            errors.push(format!("{account_name}: {e}"));
                            continue;
                        }
                    }
                };

                let acct_api = GmailApi::new(&app.client);
                match acct_api.list_messages(Some(&query), None, 10, None).await {
                    Ok(val) => {
                        let messages = val.get("messages").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                        for m in &messages {
                            let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let snippet = m.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let subtitle = if snippet.len() > 60 {
                                format!("{}...", &snippet[..57])
                            } else {
                                snippet
                            };
                            all_items.push(ListItem {
                                id: id.clone(),
                                title: format!("[{}] Message {}", label, &id[..id.len().min(10)]),
                                subtitle,
                                metadata: {
                                    let mut meta = m.clone();
                                    meta["_account"] = serde_json::json!(account_name);
                                    meta["_label"] = serde_json::json!(&label);
                                    meta
                                },
                            });
                        }
                    }
                    Err(e) => {
                        errors.push(format!("{label}: {e}"));
                    }
                }
            }

            // Switch back to original account
            if app.client.active_account_name().await != original_account {
                let _ = app.client.switch_account(&original_account).await;
            }

            app.set_items(all_items);
            app.next_page_token = None;
            app.screen = Screen::ActionView;

            let msg = format!("{} results across {} accounts", app.items.len(), all_accounts.len());
            if errors.is_empty() {
                Ok(msg)
            } else {
                Ok(format!("{} (errors: {})", msg, errors.join(", ")))
            }
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Calendar handlers ──

async fn handle_calendar(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = CalendarApi::new(&app.client);
    match action {
        0 | 1 => {
            // Today / Week View
            let now = chrono::Utc::now();
            let time_min = if action == 0 {
                now.format("%Y-%m-%dT00:00:00Z").to_string()
            } else {
                now.format("%Y-%m-%dT00:00:00Z").to_string()
            };
            let time_max = if action == 0 {
                now.format("%Y-%m-%dT23:59:59Z").to_string()
            } else {
                (now + chrono::Duration::days(7)).format("%Y-%m-%dT23:59:59Z").to_string()
            };
            let val = api.list_events("primary", Some(&time_min), Some(&time_max), None, 50, None, true, Some("startTime")).await?;
            parse_calendar_events(app, &val);
            app.service = Some(Service::Calendar);
            app.screen = Screen::ActionView;
            let label = if action == 0 { "today" } else { "this week" };
            app.set_status(format!("{} events {}", app.items.len(), label));
        }
        2 => {
            // All events
            let val = api.list_events("primary", None, None, None, 50, None, true, Some("startTime")).await?;
            parse_calendar_events(app, &val);
            app.service = Some(Service::Calendar);
            app.screen = Screen::ActionView;
        }
        3 => {
            // Quick Add
            app.input_fields = vec![InputField::new("Quick Add", "Meeting with Bob tomorrow at 3pm", true)];
            app.service = Some(Service::Calendar);
            app.screen = Screen::Input;
        }
        4 => {
            // Calendars
            let val = api.list_calendars(None).await?;
            let items = val.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                items.iter().map(|c| ListItem {
                    id: c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: c.get("summary").and_then(|v| v.as_str()).unwrap_or("Unnamed").to_string(),
                    subtitle: c.get("accessRole").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    metadata: c.clone(),
                }).collect(),
            );
            app.service = Some(Service::Calendar);
            app.screen = Screen::ActionView;
        }
        5 => {
            // Create Event
            app.input_fields = vec![
                InputField::new("Summary", "Meeting title", true),
                InputField::new("Start (RFC3339)", "2026-01-15T10:00:00-05:00", true),
                InputField::new("End (RFC3339)", "2026-01-15T11:00:00-05:00", true),
                InputField::new("Location", "Conference Room", false),
                InputField::new("Description", "Event details", false),
                InputField::new("Attendees (comma-sep)", "a@b.com,c@d.com", false),
            ];
            app.service = Some(Service::Calendar);
            app.screen = Screen::Input;
        }
        6 => {
            // ACL
            let val = api.list_acl("primary").await?;
            let items = val.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                items.iter().map(|a| ListItem {
                    id: a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: a.get("scope").and_then(|s| s.get("value")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    subtitle: a.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    metadata: a.clone(),
                }).collect(),
            );
            app.service = Some(Service::Calendar);
            app.screen = Screen::ActionView;
        }
        7 => {
            // Settings
            let val = api.list_settings().await?;
            app.detail = Some(val);
            app.service = Some(Service::Calendar);
            app.screen = Screen::ActionView;
        }
        8 => {
            // Free/Busy
            app.input_fields = vec![
                InputField::new("Calendar ID", "primary", true),
                InputField::new("Start (RFC3339)", "2026-01-15T00:00:00Z", true),
                InputField::new("End (RFC3339)", "2026-01-16T00:00:00Z", true),
            ];
            app.service = Some(Service::Calendar);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

fn parse_calendar_events(app: &mut App, val: &Value) {
    let events = val.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    app.set_items(
        events.iter().map(|e| {
            let start = e.get("start")
                .and_then(|s| s.get("dateTime").or(s.get("date")))
                .and_then(|v| v.as_str())
                .unwrap_or("No date");
            ListItem {
                id: e.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                title: e.get("summary").and_then(|v| v.as_str()).unwrap_or("(No title)").to_string(),
                subtitle: start.to_string(),
                metadata: e.clone(),
            }
        }).collect(),
    );
}

async fn submit_calendar(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = CalendarApi::new(&app.client);
    let action = app.selected_action;

    match action {
        3 => {
            // Quick Add
            api.quick_add_event("primary", &fields[0].value).await?;
            Ok("Event created via quick add!".to_string())
        }
        5 => {
            // Create Event
            let mut event = serde_json::json!({
                "summary": &fields[0].value,
                "start": { "dateTime": &fields[1].value },
                "end": { "dateTime": &fields[2].value },
            });
            if !fields[3].value.is_empty() {
                event["location"] = serde_json::json!(&fields[3].value);
            }
            if !fields[4].value.is_empty() {
                event["description"] = serde_json::json!(&fields[4].value);
            }
            if !fields[5].value.is_empty() {
                let attendees: Vec<Value> = fields[5].value.split(',')
                    .map(|e| serde_json::json!({"email": e.trim()}))
                    .collect();
                event["attendees"] = serde_json::json!(attendees);
            }
            api.create_event("primary", &event).await?;
            Ok("Event created!".to_string())
        }
        8 => {
            // Free/Busy
            let body = serde_json::json!({
                "timeMin": &fields[1].value,
                "timeMax": &fields[2].value,
                "items": [{"id": &fields[0].value}],
            });
            let val = api.query_free_busy(&body).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Free/busy loaded".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Drive handlers ──

async fn handle_drive(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = DriveApi::new(&app.client);
    match action {
        0 => {
            // My Files
            let val = api.list_files(Some("'root' in parents and trashed=false"), 20, None, Some("modifiedTime desc"), None, None).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        1 => {
            // Search
            app.input_fields = vec![InputField::new("Search Query", "name contains 'report'", true)];
            app.service = Some(Service::Drive);
            app.screen = Screen::Input;
        }
        2 => {
            // Upload (metadata only)
            app.input_fields = vec![
                InputField::new("File Path", "/path/to/file.txt", true),
                InputField::new("Folder ID (optional)", "root", false),
            ];
            app.service = Some(Service::Drive);
            app.screen = Screen::Input;
        }
        3 => {
            // Create Folder
            app.input_fields = vec![
                InputField::new("Folder Name", "New Folder", true),
                InputField::new("Parent Folder ID", "root", false),
            ];
            app.service = Some(Service::Drive);
            app.screen = Screen::Input;
        }
        4 => {
            // Shared with me
            let val = api.list_files(Some("sharedWithMe=true"), 20, None, Some("modifiedTime desc"), None, None).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        5 => {
            // Recent
            let val = api.list_files(Some("trashed=false"), 20, None, Some("viewedByMeTime desc"), None, None).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        6 => {
            // Starred
            let val = api.list_files(Some("starred=true and trashed=false"), 20, None, None, None, None).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        7 => {
            // Trash
            let val = api.list_files(Some("trashed=true"), 20, None, None, None, None).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        8 => {
            // Storage Info
            let val = api.get_about().await?;
            app.detail = Some(val);
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
            app.set_status("Drive storage info loaded");
        }
        9 => {
            // Shared Drives
            let val = api.list_shared_drives(None).await?;
            let drives = val.get("drives").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                drives.iter().map(|d| ListItem {
                    id: d.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: d.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    subtitle: "Shared Drive".to_string(),
                    metadata: d.clone(),
                }).collect(),
            );
            app.service = Some(Service::Drive);
            app.screen = Screen::ActionView;
        }
        _ => {}
    }
    Ok(())
}

fn parse_drive_files(app: &mut App, val: &Value) {
    app.next_page_token = val.get("nextPageToken").and_then(|v| v.as_str()).map(|s| s.to_string());
    let files = val.get("files").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    app.set_items(
        files.iter().map(|f| {
            let mime = f.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
            let icon = if mime.contains("folder") { "📁" }
                else if mime.contains("spreadsheet") { "📊" }
                else if mime.contains("document") { "📄" }
                else if mime.contains("presentation") { "📽" }
                else if mime.contains("form") { "📝" }
                else if mime.contains("image") { "🖼" }
                else if mime.contains("pdf") { "📕" }
                else { "📎" };
            ListItem {
                id: f.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                title: format!("{} {}", icon, f.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed")),
                subtitle: f.get("modifiedTime").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                metadata: f.clone(),
            }
        }).collect(),
    );
    app.page_info = if app.next_page_token.is_some() { "more available".to_string() } else { String::new() };
}

async fn submit_drive(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = DriveApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            // Search
            let val = api.list_files(Some(&fields[0].value), 20, None, None, None, None).await?;
            parse_drive_files(app, &val);
            app.screen = Screen::ActionView;
            Ok(format!("{} files found", app.items.len()))
        }
        2 => {
            // Upload
            let path = std::path::Path::new(&fields[0].value);
            let content = std::fs::read(path)?;
            let mime = mime_guess::from_path(path).first_or_octet_stream().to_string();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("upload");
            let mut meta = serde_json::json!({"name": name});
            if !fields[1].value.is_empty() && fields[1].value != "root" {
                meta["parents"] = serde_json::json!([&fields[1].value]);
            }
            api.upload_file(&meta, content, &mime).await?;
            Ok(format!("Uploaded {name}!"))
        }
        3 => {
            // Create Folder
            let parent = if fields[1].value.is_empty() { None } else { Some(fields[1].value.as_str()) };
            api.create_folder(&fields[0].value, parent).await?;
            Ok(format!("Folder '{}' created!", fields[0].value))
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Sheets handlers ──

async fn handle_sheets(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api_drive = DriveApi::new(&app.client);
    match action {
        0 => {
            // Open Sheet (list sheets from drive)
            let val = api_drive.list_files(
                Some("mimeType='application/vnd.google-apps.spreadsheet' and trashed=false"),
                20, None, Some("modifiedTime desc"), None, None,
            ).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Sheets);
            app.screen = Screen::ActionView;
        }
        1 => {
            // Create Sheet
            app.input_fields = vec![InputField::new("Spreadsheet Title", "New Spreadsheet", true)];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        2 => {
            // Read Range
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("Range", "Sheet1!A1:Z100", true),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        3 => {
            // Write Range
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("Range", "Sheet1!A1", true),
                InputField::new("Values (JSON array)", "[[\"a\",\"b\"],[\"c\",\"d\"]]", true).multiline(),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        4 => {
            // Append Data
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("Range", "Sheet1!A1", true),
                InputField::new("Values (JSON array)", "[[\"new\",\"row\"]]", true).multiline(),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        5 => {
            // Manage Sheets (add/delete)
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("New Sheet Title", "Sheet2", true),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        6 => {
            // Named Ranges
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("Range Name", "MyRange", true),
                InputField::new("Sheet ID (number)", "0", true),
                InputField::new("Start Row", "0", true),
                InputField::new("End Row", "10", true),
                InputField::new("Start Col", "0", true),
                InputField::new("End Col", "5", true),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        7 => {
            // Sort
            app.input_fields = vec![
                InputField::new("Spreadsheet ID", "paste spreadsheet ID", true),
                InputField::new("Sheet ID (number)", "0", true),
                InputField::new("Sort Column Index", "0", true),
                InputField::new("Ascending (true/false)", "true", true),
            ];
            app.service = Some(Service::Sheets);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_sheets(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = SheetsApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.create_spreadsheet(&fields[0].value).await?;
            let id = val.get("spreadsheetId").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(format!("Spreadsheet created: {id}"))
        }
        2 => {
            let val = api.get_values(&fields[0].value, &fields[1].value, None).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Values loaded".to_string())
        }
        3 => {
            let values: Value = serde_json::from_str(&fields[2].value)
                .map_err(|e| crate::error::VgoogError::Other(format!("Invalid JSON: {e}")))?;
            api.update_values(&fields[0].value, &fields[1].value, &values, "USER_ENTERED").await?;
            Ok("Values written!".to_string())
        }
        4 => {
            let values: Value = serde_json::from_str(&fields[2].value)
                .map_err(|e| crate::error::VgoogError::Other(format!("Invalid JSON: {e}")))?;
            api.append_values(&fields[0].value, &fields[1].value, &values, "USER_ENTERED").await?;
            Ok("Data appended!".to_string())
        }
        5 => {
            api.add_sheet(&fields[0].value, &fields[1].value).await?;
            Ok(format!("Sheet '{}' added!", fields[1].value))
        }
        6 => {
            let sheet_id: i64 = fields[2].value.parse().unwrap_or(0);
            let sr: i64 = fields[3].value.parse().unwrap_or(0);
            let er: i64 = fields[4].value.parse().unwrap_or(10);
            let sc: i64 = fields[5].value.parse().unwrap_or(0);
            let ec: i64 = fields[6].value.parse().unwrap_or(5);
            api.create_named_range(&fields[0].value, &fields[1].value, sheet_id, sr, er, sc, ec).await?;
            Ok("Named range created!".to_string())
        }
        7 => {
            let sheet_id: i64 = fields[1].value.parse().unwrap_or(0);
            let sort_col: i64 = fields[2].value.parse().unwrap_or(0);
            let ascending = fields[3].value.to_lowercase() == "true";
            api.sort_range(&fields[0].value, sheet_id, 0, 1000, 0, 26, sort_col, ascending).await?;
            Ok("Range sorted!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Docs handlers ──

async fn handle_docs(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api_drive = DriveApi::new(&app.client);
    match action {
        0 => {
            let val = api_drive.list_files(
                Some("mimeType='application/vnd.google-apps.document' and trashed=false"),
                20, None, Some("modifiedTime desc"), None, None,
            ).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Docs);
            app.screen = Screen::ActionView;
        }
        1 => {
            app.input_fields = vec![InputField::new("Document Title", "New Document", true)];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        2 => {
            app.input_fields = vec![
                InputField::new("Document ID", "paste doc ID", true),
                InputField::new("Text", "Hello, World!", true).multiline(),
                InputField::new("Insert at index", "1", true),
            ];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        3 => {
            app.input_fields = vec![
                InputField::new("Document ID", "paste doc ID", true),
                InputField::new("Find", "old text", true),
                InputField::new("Replace With", "new text", true),
            ];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        4 => {
            // Formatting
            app.input_fields = vec![
                InputField::new("Document ID", "paste doc ID", true),
                InputField::new("Start Index", "1", true),
                InputField::new("End Index", "10", true),
                InputField::new("Bold (true/false)", "true", false),
                InputField::new("Italic (true/false)", "", false),
                InputField::new("Font Size (pt)", "", false),
            ];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        5 => {
            // Headers/Footers
            app.input_fields = vec![
                InputField::new("Document ID", "paste doc ID", true),
                InputField::new("Type (header/footer)", "header", true),
            ];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        6 => {
            // Tables
            app.input_fields = vec![
                InputField::new("Document ID", "paste doc ID", true),
                InputField::new("Rows", "3", true),
                InputField::new("Columns", "3", true),
                InputField::new("Insert at index", "1", true),
            ];
            app.service = Some(Service::Docs);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_docs(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = DocsApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.create_document(&fields[0].value).await?;
            let id = val.get("documentId").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(format!("Document created: {id}"))
        }
        2 => {
            let idx: i64 = fields[2].value.parse().unwrap_or(1);
            api.insert_text(&fields[0].value, &fields[1].value, idx).await?;
            Ok("Text inserted!".to_string())
        }
        3 => {
            api.replace_all_text(&fields[0].value, &fields[1].value, &fields[2].value, true).await?;
            Ok("Text replaced!".to_string())
        }
        4 => {
            let start: i64 = fields[1].value.parse().unwrap_or(1);
            let end: i64 = fields[2].value.parse().unwrap_or(10);
            let bold = if fields[3].value.is_empty() { None } else { Some(fields[3].value == "true") };
            let italic = if fields[4].value.is_empty() { None } else { Some(fields[4].value == "true") };
            let font_size = if fields[5].value.is_empty() { None } else { fields[5].value.parse().ok() };
            api.update_text_style(&fields[0].value, start, end, bold, italic, None, font_size).await?;
            Ok("Formatting applied!".to_string())
        }
        5 => {
            if fields[1].value == "header" {
                api.create_header(&fields[0].value, None).await?;
                Ok("Header created!".to_string())
            } else {
                api.create_footer(&fields[0].value, None).await?;
                Ok("Footer created!".to_string())
            }
        }
        6 => {
            let rows: i64 = fields[1].value.parse().unwrap_or(3);
            let cols: i64 = fields[2].value.parse().unwrap_or(3);
            let idx: i64 = fields[3].value.parse().unwrap_or(1);
            api.insert_table(&fields[0].value, rows, cols, idx).await?;
            Ok("Table inserted!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Slides handlers ──

async fn handle_slides(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api_drive = DriveApi::new(&app.client);
    match action {
        0 => {
            let val = api_drive.list_files(
                Some("mimeType='application/vnd.google-apps.presentation' and trashed=false"),
                20, None, Some("modifiedTime desc"), None, None,
            ).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Slides);
            app.screen = Screen::ActionView;
        }
        1 => {
            app.input_fields = vec![InputField::new("Presentation Title", "New Presentation", true)];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        2 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Layout", "BLANK", true),
                InputField::new("Insert at index (optional)", "", false),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        3 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Find Text", "placeholder", true),
                InputField::new("Replace With", "actual text", true),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        4 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Page/Slide ID", "page ID", true),
                InputField::new("Shape Type", "TEXT_BOX", true),
                InputField::new("X (pt)", "100", true),
                InputField::new("Y (pt)", "100", true),
                InputField::new("Width (pt)", "300", true),
                InputField::new("Height (pt)", "100", true),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        5 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Page/Slide ID", "page ID", true),
                InputField::new("Image URL", "https://example.com/img.png", true),
                InputField::new("X (pt)", "100", true),
                InputField::new("Y (pt)", "100", true),
                InputField::new("Width (pt)", "300", true),
                InputField::new("Height (pt)", "200", true),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        6 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Page/Slide ID", "page ID", true),
                InputField::new("Rows", "3", true),
                InputField::new("Columns", "3", true),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        7 => {
            app.input_fields = vec![
                InputField::new("Presentation ID", "paste ID", true),
                InputField::new("Slide ID", "slide object ID", true),
                InputField::new("Notes Text", "Speaker notes here", true).multiline(),
            ];
            app.service = Some(Service::Slides);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_slides(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = SlidesApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.create_presentation(&fields[0].value).await?;
            let id = val.get("presentationId").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(format!("Presentation created: {id}"))
        }
        2 => {
            let idx = if fields[2].value.is_empty() { None } else { fields[2].value.parse().ok() };
            api.create_slide(&fields[0].value, &fields[1].value, idx).await?;
            Ok("Slide added!".to_string())
        }
        3 => {
            api.replace_all_text(&fields[0].value, &fields[1].value, &fields[2].value, true).await?;
            Ok("Text replaced!".to_string())
        }
        4 => {
            let x: f64 = fields[3].value.parse().unwrap_or(100.0);
            let y: f64 = fields[4].value.parse().unwrap_or(100.0);
            let w: f64 = fields[5].value.parse().unwrap_or(300.0);
            let h: f64 = fields[6].value.parse().unwrap_or(100.0);
            api.create_shape(&fields[0].value, &fields[1].value, &fields[2].value, x, y, w, h).await?;
            Ok("Shape created!".to_string())
        }
        5 => {
            let x: f64 = fields[3].value.parse().unwrap_or(100.0);
            let y: f64 = fields[4].value.parse().unwrap_or(100.0);
            let w: f64 = fields[5].value.parse().unwrap_or(300.0);
            let h: f64 = fields[6].value.parse().unwrap_or(200.0);
            api.create_image(&fields[0].value, &fields[1].value, &fields[2].value, x, y, w, h).await?;
            Ok("Image inserted!".to_string())
        }
        6 => {
            let rows: i64 = fields[2].value.parse().unwrap_or(3);
            let cols: i64 = fields[3].value.parse().unwrap_or(3);
            api.create_table(&fields[0].value, &fields[1].value, rows, cols).await?;
            Ok("Table created!".to_string())
        }
        7 => {
            api.create_speaker_notes(&fields[0].value, &fields[1].value, &fields[2].value).await?;
            Ok("Speaker notes added!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Forms handlers ──

async fn handle_forms(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api_drive = DriveApi::new(&app.client);
    match action {
        0 => {
            let val = api_drive.list_files(
                Some("mimeType='application/vnd.google-apps.form' and trashed=false"),
                20, None, Some("modifiedTime desc"), None, None,
            ).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::Forms);
            app.screen = Screen::ActionView;
        }
        1 => {
            app.input_fields = vec![
                InputField::new("Form Title", "New Form", true),
                InputField::new("Document Title", "My Survey", true),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        2 => {
            app.input_fields = vec![
                InputField::new("Form ID", "paste form ID", true),
                InputField::new("Question Title", "Your question here", true),
                InputField::new("Type (text/choice/scale/date/time)", "text", true),
                InputField::new("Required (true/false)", "true", true),
                InputField::new("Options (comma-sep, for choice)", "Option A,Option B,Option C", false),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        3 => {
            app.input_fields = vec![
                InputField::new("Form ID", "paste form ID", true),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        4 => {
            app.input_fields = vec![
                InputField::new("Form ID", "paste form ID", true),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        5 => {
            app.input_fields = vec![
                InputField::new("Form ID", "paste form ID", true),
                InputField::new("Title", "", false),
                InputField::new("Description", "", false),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        6 => {
            // Grid question
            app.input_fields = vec![
                InputField::new("Form ID", "paste form ID", true),
                InputField::new("Title", "Rate the following", true),
                InputField::new("Rows (comma-sep)", "Quality,Speed,Price", true),
                InputField::new("Columns (comma-sep)", "1,2,3,4,5", true),
            ];
            app.service = Some(Service::Forms);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_forms(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = FormsApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.create_form(&fields[0].value, &fields[1].value).await?;
            let id = val.get("formId").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(format!("Form created: {id}"))
        }
        2 => {
            let qtype = &fields[2].value;
            let required = fields[3].value == "true";
            match qtype.as_str() {
                "text" => {
                    api.add_text_question(&fields[0].value, &fields[1].value, required, 0, false).await?;
                }
                "choice" => {
                    let opts: Vec<&str> = fields[4].value.split(',').map(|s| s.trim()).collect();
                    api.add_choice_question(&fields[0].value, &fields[1].value, required, 0, "RADIO", &opts).await?;
                }
                "scale" => {
                    api.add_scale_question(&fields[0].value, &fields[1].value, required, 0, 1, 5, "Low", "High").await?;
                }
                "date" => {
                    api.add_date_question(&fields[0].value, &fields[1].value, required, 0, false, true).await?;
                }
                "time" => {
                    api.add_time_question(&fields[0].value, &fields[1].value, required, 0, false).await?;
                }
                _ => {
                    api.add_text_question(&fields[0].value, &fields[1].value, required, 0, false).await?;
                }
            }
            Ok("Question added!".to_string())
        }
        3 => {
            let val = api.list_responses(&fields[0].value, None, 50).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Responses loaded".to_string())
        }
        4 => {
            let val = api.list_watches(&fields[0].value).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Watches loaded".to_string())
        }
        5 => {
            let title = if fields[1].value.is_empty() { None } else { Some(fields[1].value.as_str()) };
            let desc = if fields[2].value.is_empty() { None } else { Some(fields[2].value.as_str()) };
            api.update_form_info(&fields[0].value, title, desc).await?;
            Ok("Form info updated!".to_string())
        }
        6 => {
            let rows: Vec<&str> = fields[2].value.split(',').map(|s| s.trim()).collect();
            let cols: Vec<&str> = fields[3].value.split(',').map(|s| s.trim()).collect();
            api.add_grid_question(&fields[0].value, &fields[1].value, true, 0, &rows, &cols).await?;
            Ok("Grid question added!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Tasks handlers ──

async fn handle_tasks(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = TasksApi::new(&app.client);
    match action {
        0 => {
            // Task Lists
            let val = api.list_task_lists(20, None).await?;
            let items = val.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                items.iter().map(|t| ListItem {
                    id: t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: t.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string(),
                    subtitle: t.get("updated").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    metadata: t.clone(),
                }).collect(),
            );
            app.service = Some(Service::Tasks);
            app.screen = Screen::ActionView;
        }
        1 => {
            // View Tasks (need task list selection)
            app.input_fields = vec![InputField::new("Task List ID", "paste task list ID", true)];
            app.service = Some(Service::Tasks);
            app.screen = Screen::Input;
        }
        2 => {
            // Create Task
            app.input_fields = vec![
                InputField::new("Task List ID", "paste task list ID", true),
                InputField::new("Title", "Buy groceries", true),
                InputField::new("Notes", "Milk, eggs, bread", false),
                InputField::new("Due (RFC3339)", "2026-01-20T00:00:00Z", false),
            ];
            app.service = Some(Service::Tasks);
            app.screen = Screen::Input;
        }
        3 => {
            // Complete/Toggle
            app.input_fields = vec![
                InputField::new("Task List ID", "paste task list ID", true),
                InputField::new("Task ID", "paste task ID", true),
                InputField::new("Action (complete/uncomplete)", "complete", true),
            ];
            app.service = Some(Service::Tasks);
            app.screen = Screen::Input;
        }
        4 => {
            // Move Task
            app.input_fields = vec![
                InputField::new("Task List ID", "paste task list ID", true),
                InputField::new("Task ID", "paste task ID", true),
                InputField::new("Parent Task ID (optional)", "", false),
                InputField::new("Previous Task ID (optional)", "", false),
            ];
            app.service = Some(Service::Tasks);
            app.screen = Screen::Input;
        }
        5 => {
            // Clear Completed
            app.input_fields = vec![InputField::new("Task List ID", "paste task list ID", true)];
            app.service = Some(Service::Tasks);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_tasks(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = TasksApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.list_tasks(&fields[0].value, 50, None, true, false, false, None, None).await?;
            let items = val.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                items.iter().map(|t| {
                    let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    let icon = if status == "completed" { "✓" } else { "○" };
                    ListItem {
                        id: t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: format!("{} {}", icon, t.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled")),
                        subtitle: t.get("due").and_then(|v| v.as_str()).unwrap_or("No due date").to_string(),
                        metadata: {
                            let mut m = t.clone();
                            m["taskListId"] = serde_json::json!(&fields[0].value);
                            m
                        },
                    }
                }).collect(),
            );
            app.screen = Screen::ActionView;
            Ok(format!("{} tasks loaded", app.items.len()))
        }
        2 => {
            let notes = if fields[2].value.is_empty() { None } else { Some(fields[2].value.as_str()) };
            let due = if fields[3].value.is_empty() { None } else { Some(fields[3].value.as_str()) };
            api.create_task(&fields[0].value, &fields[1].value, notes, due, None, None).await?;
            Ok("Task created!".to_string())
        }
        3 => {
            if fields[2].value == "complete" {
                api.complete_task(&fields[0].value, &fields[1].value).await?;
                Ok("Task completed!".to_string())
            } else {
                api.uncomplete_task(&fields[0].value, &fields[1].value).await?;
                Ok("Task uncompleted!".to_string())
            }
        }
        4 => {
            let parent = if fields[2].value.is_empty() { None } else { Some(fields[2].value.as_str()) };
            let previous = if fields[3].value.is_empty() { None } else { Some(fields[3].value.as_str()) };
            api.move_task(&fields[0].value, &fields[1].value, parent, previous).await?;
            Ok("Task moved!".to_string())
        }
        5 => {
            api.clear_completed(&fields[0].value).await?;
            Ok("Completed tasks cleared!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── People handlers ──

async fn handle_people(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = PeopleApi::new(&app.client);
    match action {
        0 => {
            let val = api.list_contacts(20, None, "names,emailAddresses,phoneNumbers,organizations", Some("LAST_NAME_ASCENDING")).await?;
            let connections = val.get("connections").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                connections.iter().map(|c| {
                    let name = c.get("names")
                        .and_then(|n| n.as_array())
                        .and_then(|a| a.first())
                        .and_then(|n| n.get("displayName"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed");
                    let email = c.get("emailAddresses")
                        .and_then(|e| e.as_array())
                        .and_then(|a| a.first())
                        .and_then(|e| e.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    ListItem {
                        id: c.get("resourceName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: name.to_string(),
                        subtitle: email.to_string(),
                        metadata: c.clone(),
                    }
                }).collect(),
            );
            app.service = Some(Service::People);
            app.screen = Screen::ActionView;
            app.set_status(format!("{} contacts loaded", app.items.len()));
        }
        1 => {
            app.input_fields = vec![InputField::new("Search Query", "John", true)];
            app.service = Some(Service::People);
            app.screen = Screen::Input;
        }
        2 => {
            app.input_fields = vec![
                InputField::new("First Name", "John", true),
                InputField::new("Last Name", "Doe", true),
                InputField::new("Email", "john@example.com", false),
                InputField::new("Phone", "+1-555-0100", false),
                InputField::new("Organization", "Acme Corp", false),
            ];
            app.service = Some(Service::People);
            app.screen = Screen::Input;
        }
        3 => {
            let val = api.list_contact_groups(20, None).await?;
            let groups = val.get("contactGroups").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                groups.iter().map(|g| ListItem {
                    id: g.get("resourceName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: g.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed").to_string(),
                    subtitle: format!("{} members", g.get("memberCount").and_then(|v| v.as_i64()).unwrap_or(0)),
                    metadata: g.clone(),
                }).collect(),
            );
            app.service = Some(Service::People);
            app.screen = Screen::ActionView;
        }
        4 => {
            let val = api.list_other_contacts(20, None).await?;
            let others = val.get("otherContacts").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                others.iter().map(|c| {
                    let name = c.get("names")
                        .and_then(|n| n.as_array())
                        .and_then(|a| a.first())
                        .and_then(|n| n.get("displayName"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed");
                    ListItem {
                        id: c.get("resourceName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: name.to_string(),
                        subtitle: "Other contact".to_string(),
                        metadata: c.clone(),
                    }
                }).collect(),
            );
            app.service = Some(Service::People);
            app.screen = Screen::ActionView;
        }
        5 => {
            app.input_fields = vec![InputField::new("Search Query", "Jane", true)];
            app.service = Some(Service::People);
            app.screen = Screen::Input;
        }
        _ => {}
    }
    Ok(())
}

async fn submit_people(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = PeopleApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let val = api.search_contacts(&fields[0].value, 20).await?;
            let results = val.get("results").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            app.set_items(
                results.iter().filter_map(|r| {
                    let person = r.get("person")?;
                    let name = person.get("names")
                        .and_then(|n| n.as_array())
                        .and_then(|a| a.first())
                        .and_then(|n| n.get("displayName"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed");
                    Some(ListItem {
                        id: person.get("resourceName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        title: name.to_string(),
                        subtitle: "Search result".to_string(),
                        metadata: person.clone(),
                    })
                }).collect(),
            );
            app.screen = Screen::ActionView;
            Ok(format!("{} contacts found", app.items.len()))
        }
        2 => {
            let mut person = serde_json::json!({
                "names": [{"givenName": &fields[0].value, "familyName": &fields[1].value}],
            });
            if !fields[2].value.is_empty() {
                person["emailAddresses"] = serde_json::json!([{"value": &fields[2].value}]);
            }
            if !fields[3].value.is_empty() {
                person["phoneNumbers"] = serde_json::json!([{"value": &fields[3].value}]);
            }
            if !fields[4].value.is_empty() {
                person["organizations"] = serde_json::json!([{"name": &fields[4].value}]);
            }
            api.create_contact(&person).await?;
            Ok("Contact created!".to_string())
        }
        5 => {
            let val = api.search_directory(&fields[0].value, 20, None).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Directory search results loaded".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}

// ── Apps Script handlers ──

async fn handle_apps_script(app: &mut App, action: usize) -> crate::error::Result<()> {
    let api = AppsScriptApi::new(&app.client);
    match action {
        0 => {
            // List projects via Drive
            let api_drive = DriveApi::new(&app.client);
            let val = api_drive.list_files(
                Some("mimeType='application/vnd.google-apps.script' and trashed=false"),
                20, None, Some("modifiedTime desc"), None, None,
            ).await?;
            parse_drive_files(app, &val);
            app.service = Some(Service::AppsScript);
            app.screen = Screen::ActionView;
        }
        1 => {
            app.input_fields = vec![
                InputField::new("Project Title", "My Script", true),
                InputField::new("Parent Doc ID (optional)", "", false),
            ];
            app.service = Some(Service::AppsScript);
            app.screen = Screen::Input;
        }
        2 => {
            app.input_fields = vec![
                InputField::new("Script ID", "paste script ID", true),
                InputField::new("File Name", "Code", true),
                InputField::new("Source Code", "function main() { Logger.log('Hello'); }", true).multiline(),
            ];
            app.service = Some(Service::AppsScript);
            app.screen = Screen::Input;
        }
        3 => {
            app.input_fields = vec![
                InputField::new("Script ID", "paste script ID", true),
            ];
            app.service = Some(Service::AppsScript);
            app.screen = Screen::Input;
        }
        4 => {
            app.input_fields = vec![
                InputField::new("Script ID", "paste script ID", true),
            ];
            app.service = Some(Service::AppsScript);
            app.screen = Screen::Input;
        }
        5 => {
            app.input_fields = vec![
                InputField::new("Script ID", "paste script ID", true),
                InputField::new("Function Name", "main", true),
                InputField::new("Parameters (JSON array)", "[]", false),
                InputField::new("Dev Mode (true/false)", "true", false),
            ];
            app.service = Some(Service::AppsScript);
            app.screen = Screen::Input;
        }
        6 => {
            let val = api.list_processes(20, None).await?;
            app.detail = Some(val);
            app.service = Some(Service::AppsScript);
            app.screen = Screen::ActionView;
            app.set_status("Processes loaded");
        }
        _ => {}
    }
    Ok(())
}

async fn submit_apps_script(app: &mut App, fields: &[InputField]) -> crate::error::Result<String> {
    let api = AppsScriptApi::new(&app.client);
    let action = app.selected_action;

    match action {
        1 => {
            let parent = if fields[1].value.is_empty() { None } else { Some(fields[1].value.as_str()) };
            let val = api.create_project(&fields[0].value, parent).await?;
            let id = val.get("scriptId").and_then(|v| v.as_str()).unwrap_or("unknown");
            Ok(format!("Project created: {id}"))
        }
        2 => {
            let files = vec![
                AppsScriptApi::make_server_js_file(&fields[1].value, &fields[2].value),
                AppsScriptApi::make_manifest("America/New_York", &serde_json::json!({})),
            ];
            api.update_content(&fields[0].value, &files).await?;
            Ok("Code updated!".to_string())
        }
        3 => {
            let val = api.list_versions(&fields[0].value, 20, None).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Versions loaded".to_string())
        }
        4 => {
            let val = api.list_deployments(&fields[0].value, 20, None).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Deployments loaded".to_string())
        }
        5 => {
            let params: Option<Vec<Value>> = if fields[2].value.is_empty() || fields[2].value == "[]" {
                None
            } else {
                serde_json::from_str(&fields[2].value).ok()
            };
            let dev_mode = fields[3].value == "true";
            let val = api.run(
                &fields[0].value,
                &fields[1].value,
                params.as_deref(),
                dev_mode,
            ).await?;
            app.detail = Some(val);
            app.screen = Screen::ActionView;
            Ok("Function executed!".to_string())
        }
        _ => Ok("Action completed".to_string()),
    }
}
