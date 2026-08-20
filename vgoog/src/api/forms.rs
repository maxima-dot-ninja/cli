use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://forms.googleapis.com/v1/forms";

pub struct FormsApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> FormsApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Forms ──

    pub async fn create_form(&self, title: &str, document_title: &str) -> Result<Value> {
        let url = BASE.to_string();
        self.client
            .post(
                &url,
                &json!({
                    "info": {
                        "title": title,
                        "documentTitle": document_title,
                    }
                }),
            )
            .await
    }

    pub async fn get_form(&self, form_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{form_id}");
        self.client.get(&url).await
    }

    pub async fn batch_update(&self, form_id: &str, requests: &[Value]) -> Result<Value> {
        let url = format!("{BASE}/{form_id}:batchUpdate");
        self.client
            .post(
                &url,
                &json!({
                    "includeFormInResponse": true,
                    "requests": requests,
                }),
            )
            .await
    }

    // ── Responses ──

    pub async fn list_responses(
        &self,
        form_id: &str,
        page_token: Option<&str>,
        page_size: u32,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/{form_id}/responses?pageSize={page_size}");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_response(&self, form_id: &str, response_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{form_id}/responses/{response_id}");
        self.client.get(&url).await
    }

    // ── Watches ──

    pub async fn create_watch(
        &self,
        form_id: &str,
        event_type: &str,
        topic_name: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/{form_id}/watches");
        self.client
            .post(
                &url,
                &json!({
                    "watch": {
                        "target": {
                            "topic": { "topicName": topic_name }
                        },
                        "eventType": event_type,
                    }
                }),
            )
            .await
    }

    pub async fn list_watches(&self, form_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{form_id}/watches");
        self.client.get(&url).await
    }

    pub async fn delete_watch(&self, form_id: &str, watch_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{form_id}/watches/{watch_id}");
        self.client.delete(&url).await
    }

    pub async fn renew_watch(&self, form_id: &str, watch_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{form_id}/watches/{watch_id}:renew");
        self.client.post_empty(&url).await
    }

    // ── Convenience: Add items ──

    pub async fn add_text_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        paragraph: bool,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "textQuestion": {
                                    "paragraph": paragraph,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_choice_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        choice_type: &str,
        options: &[&str],
    ) -> Result<Value> {
        let opts: Vec<Value> = options.iter().map(|o| json!({ "value": o })).collect();
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "choiceQuestion": {
                                    "type": choice_type,
                                    "options": opts,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_scale_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        low: i64,
        high: i64,
        low_label: &str,
        high_label: &str,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "scaleQuestion": {
                                    "low": low,
                                    "high": high,
                                    "lowLabel": low_label,
                                    "highLabel": high_label,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_date_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        include_time: bool,
        include_year: bool,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "dateQuestion": {
                                    "includeTime": include_time,
                                    "includeYear": include_year,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_time_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        include_duration: bool,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "timeQuestion": {
                                    "duration": include_duration,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_section_header(
        &self,
        form_id: &str,
        title: &str,
        description: &str,
        index: i64,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "description": description,
                        "pageBreakItem": {}
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn delete_item(&self, form_id: &str, index: i64) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "deleteItem": {
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn move_item(
        &self,
        form_id: &str,
        original_index: i64,
        new_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "moveItem": {
                    "originalLocation": { "index": original_index },
                    "newLocation": { "index": new_index },
                }
            })],
        )
        .await
    }

    pub async fn update_form_info(
        &self,
        form_id: &str,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Result<Value> {
        let mut info = json!({});
        let mut mask = Vec::new();
        if let Some(t) = title {
            info["title"] = json!(t);
            mask.push("title");
        }
        if let Some(d) = description {
            info["description"] = json!(d);
            mask.push("description");
        }
        self.batch_update(
            form_id,
            &[json!({
                "updateFormInfo": {
                    "info": info,
                    "updateMask": mask.join(","),
                }
            })],
        )
        .await
    }

    pub async fn update_settings(
        &self,
        form_id: &str,
        settings: &Value,
        update_mask: &str,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "updateSettings": {
                    "settings": settings,
                    "updateMask": update_mask,
                }
            })],
        )
        .await
    }

    pub async fn add_file_upload_question(
        &self,
        form_id: &str,
        title: &str,
        required: bool,
        index: i64,
        max_files: i64,
        max_file_size: &str,
    ) -> Result<Value> {
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionItem": {
                            "question": {
                                "required": required,
                                "fileUploadQuestion": {
                                    "folderId": "",
                                    "maxFiles": max_files,
                                    "maxFileSize": max_file_size,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn add_grid_question(
        &self,
        form_id: &str,
        title: &str,
        _required: bool,
        index: i64,
        rows: &[&str],
        columns: &[&str],
    ) -> Result<Value> {
        let row_questions: Vec<Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "title": r,
                    "rowQuestion": {
                        "title": r,
                    }
                })
            })
            .collect();
        let col_options: Vec<Value> = columns.iter().map(|c| json!({ "value": c })).collect();
        self.batch_update(
            form_id,
            &[json!({
                "createItem": {
                    "item": {
                        "title": title,
                        "questionGroupItem": {
                            "questions": row_questions,
                            "grid": {
                                "columns": {
                                    "type": "RADIO",
                                    "options": col_options,
                                }
                            }
                        }
                    },
                    "location": { "index": index }
                }
            })],
        )
        .await
    }
}
