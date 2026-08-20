use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://docs.googleapis.com/v1/documents";

pub struct DocsApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> DocsApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    pub async fn create_document(&self, title: &str) -> Result<Value> {
        let url = BASE.to_string();
        self.client
            .post(&url, &json!({ "title": title }))
            .await
    }

    pub async fn get_document(&self, document_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{document_id}");
        self.client.get(&url).await
    }

    pub async fn batch_update(
        &self,
        document_id: &str,
        requests: &[Value],
    ) -> Result<Value> {
        let url = format!("{BASE}/{document_id}:batchUpdate");
        self.client
            .post(&url, &json!({ "requests": requests }))
            .await
    }

    // ── Convenience methods ──

    pub async fn insert_text(
        &self,
        document_id: &str,
        text: &str,
        index: i64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "insertText": {
                    "text": text,
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn delete_content(
        &self,
        document_id: &str,
        start_index: i64,
        end_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "deleteContentRange": {
                    "range": {
                        "startIndex": start_index,
                        "endIndex": end_index,
                    }
                }
            })],
        )
        .await
    }

    pub async fn insert_table(
        &self,
        document_id: &str,
        rows: i64,
        cols: i64,
        index: i64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "insertTable": {
                    "rows": rows,
                    "columns": cols,
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn insert_inline_image(
        &self,
        document_id: &str,
        uri: &str,
        index: i64,
        width_pt: f64,
        height_pt: f64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "insertInlineImage": {
                    "uri": uri,
                    "location": { "index": index },
                    "objectSize": {
                        "width": { "magnitude": width_pt, "unit": "PT" },
                        "height": { "magnitude": height_pt, "unit": "PT" },
                    }
                }
            })],
        )
        .await
    }

    pub async fn update_text_style(
        &self,
        document_id: &str,
        start_index: i64,
        end_index: i64,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        font_size: Option<f64>,
    ) -> Result<Value> {
        let mut style = json!({});
        let mut fields = Vec::new();
        if let Some(b) = bold {
            style["bold"] = json!(b);
            fields.push("bold");
        }
        if let Some(i) = italic {
            style["italic"] = json!(i);
            fields.push("italic");
        }
        if let Some(u) = underline {
            style["underline"] = json!(u);
            fields.push("underline");
        }
        if let Some(fs) = font_size {
            style["fontSize"] = json!({ "magnitude": fs, "unit": "PT" });
            fields.push("fontSize");
        }
        self.batch_update(
            document_id,
            &[json!({
                "updateTextStyle": {
                    "range": {
                        "startIndex": start_index,
                        "endIndex": end_index,
                    },
                    "textStyle": style,
                    "fields": fields.join(","),
                }
            })],
        )
        .await
    }

    pub async fn update_paragraph_style(
        &self,
        document_id: &str,
        start_index: i64,
        end_index: i64,
        named_style: &str,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "updateParagraphStyle": {
                    "range": {
                        "startIndex": start_index,
                        "endIndex": end_index,
                    },
                    "paragraphStyle": {
                        "namedStyleType": named_style,
                    },
                    "fields": "namedStyleType",
                }
            })],
        )
        .await
    }

    pub async fn replace_all_text(
        &self,
        document_id: &str,
        find: &str,
        replace: &str,
        match_case: bool,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "replaceAllText": {
                    "containsText": {
                        "text": find,
                        "matchCase": match_case,
                    },
                    "replaceText": replace,
                }
            })],
        )
        .await
    }

    pub async fn create_named_range(
        &self,
        document_id: &str,
        name: &str,
        start_index: i64,
        end_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "createNamedRange": {
                    "name": name,
                    "range": {
                        "startIndex": start_index,
                        "endIndex": end_index,
                    }
                }
            })],
        )
        .await
    }

    pub async fn insert_page_break(
        &self,
        document_id: &str,
        index: i64,
    ) -> Result<Value> {
        self.batch_update(
            document_id,
            &[json!({
                "insertPageBreak": {
                    "location": { "index": index }
                }
            })],
        )
        .await
    }

    pub async fn create_header(
        &self,
        document_id: &str,
        section_idx: Option<i64>,
    ) -> Result<Value> {
        let mut req = json!({
            "createHeader": {
                "type": "DEFAULT",
            }
        });
        if let Some(idx) = section_idx {
            req["createHeader"]["sectionBreakLocation"] = json!({ "index": idx });
        }
        self.batch_update(document_id, &[req]).await
    }

    pub async fn create_footer(
        &self,
        document_id: &str,
        section_idx: Option<i64>,
    ) -> Result<Value> {
        let mut req = json!({
            "createFooter": {
                "type": "DEFAULT",
            }
        });
        if let Some(idx) = section_idx {
            req["createFooter"]["sectionBreakLocation"] = json!({ "index": idx });
        }
        self.batch_update(document_id, &[req]).await
    }
}
