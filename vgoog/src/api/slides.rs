use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://slides.googleapis.com/v1/presentations";

pub struct SlidesApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> SlidesApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    pub async fn create_presentation(&self, title: &str) -> Result<Value> {
        let url = BASE.to_string();
        self.client
            .post(&url, &json!({ "title": title }))
            .await
    }

    pub async fn get_presentation(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/{id}");
        self.client.get(&url).await
    }

    pub async fn get_page(&self, presentation_id: &str, page_id: &str) -> Result<Value> {
        let url = format!("{BASE}/{presentation_id}/pages/{page_id}");
        self.client.get(&url).await
    }

    pub async fn get_page_thumbnail(
        &self,
        presentation_id: &str,
        page_id: &str,
    ) -> Result<Value> {
        let url = format!(
            "{BASE}/{presentation_id}/pages/{page_id}/thumbnail?thumbnailProperties.mimeType=PNG"
        );
        self.client.get(&url).await
    }

    pub async fn batch_update(
        &self,
        presentation_id: &str,
        requests: &[Value],
    ) -> Result<Value> {
        let url = format!("{BASE}/{presentation_id}:batchUpdate");
        self.client
            .post(&url, &json!({ "requests": requests }))
            .await
    }

    // ── Convenience methods ──

    pub async fn create_slide(
        &self,
        presentation_id: &str,
        layout: &str,
        insertion_index: Option<i64>,
    ) -> Result<Value> {
        let slide_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let mut req = json!({
            "createSlide": {
                "objectId": slide_id,
                "slideLayoutReference": {
                    "predefinedLayout": layout,
                },
            }
        });
        if let Some(idx) = insertion_index {
            req["createSlide"]["insertionIndex"] = json!(idx);
        }
        self.batch_update(presentation_id, &[req]).await
    }

    pub async fn delete_slide(&self, presentation_id: &str, slide_id: &str) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "deleteObject": { "objectId": slide_id }
            })],
        )
        .await
    }

    pub async fn duplicate_slide(
        &self,
        presentation_id: &str,
        slide_id: &str,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "duplicateObject": { "objectId": slide_id }
            })],
        )
        .await
    }

    pub async fn move_slide(
        &self,
        presentation_id: &str,
        slide_id: &str,
        insertion_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "updateSlidesPosition": {
                    "slideObjectIds": [slide_id],
                    "insertionIndex": insertion_index,
                }
            })],
        )
        .await
    }

    pub async fn insert_text(
        &self,
        presentation_id: &str,
        object_id: &str,
        text: &str,
        insertion_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "insertText": {
                    "objectId": object_id,
                    "text": text,
                    "insertionIndex": insertion_index,
                }
            })],
        )
        .await
    }

    pub async fn delete_text(
        &self,
        presentation_id: &str,
        object_id: &str,
        start_index: i64,
        end_index: i64,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "deleteText": {
                    "objectId": object_id,
                    "textRange": {
                        "type": "FIXED_RANGE",
                        "startIndex": start_index,
                        "endIndex": end_index,
                    }
                }
            })],
        )
        .await
    }

    pub async fn replace_all_text(
        &self,
        presentation_id: &str,
        find: &str,
        replace: &str,
        match_case: bool,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
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

    pub async fn create_shape(
        &self,
        presentation_id: &str,
        page_id: &str,
        shape_type: &str,
        x_pt: f64,
        y_pt: f64,
        width_pt: f64,
        height_pt: f64,
    ) -> Result<Value> {
        let shape_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        self.batch_update(
            presentation_id,
            &[json!({
                "createShape": {
                    "objectId": shape_id,
                    "shapeType": shape_type,
                    "elementProperties": {
                        "pageObjectId": page_id,
                        "size": {
                            "width": { "magnitude": width_pt, "unit": "PT" },
                            "height": { "magnitude": height_pt, "unit": "PT" },
                        },
                        "transform": {
                            "scaleX": 1.0,
                            "scaleY": 1.0,
                            "translateX": x_pt,
                            "translateY": y_pt,
                            "unit": "PT",
                        }
                    }
                }
            })],
        )
        .await
    }

    pub async fn create_image(
        &self,
        presentation_id: &str,
        page_id: &str,
        url: &str,
        x_pt: f64,
        y_pt: f64,
        width_pt: f64,
        height_pt: f64,
    ) -> Result<Value> {
        let img_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        self.batch_update(
            presentation_id,
            &[json!({
                "createImage": {
                    "objectId": img_id,
                    "url": url,
                    "elementProperties": {
                        "pageObjectId": page_id,
                        "size": {
                            "width": { "magnitude": width_pt, "unit": "PT" },
                            "height": { "magnitude": height_pt, "unit": "PT" },
                        },
                        "transform": {
                            "scaleX": 1.0,
                            "scaleY": 1.0,
                            "translateX": x_pt,
                            "translateY": y_pt,
                            "unit": "PT",
                        }
                    }
                }
            })],
        )
        .await
    }

    pub async fn create_table(
        &self,
        presentation_id: &str,
        page_id: &str,
        rows: i64,
        cols: i64,
    ) -> Result<Value> {
        let table_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        self.batch_update(
            presentation_id,
            &[json!({
                "createTable": {
                    "objectId": table_id,
                    "elementProperties": {
                        "pageObjectId": page_id,
                    },
                    "rows": rows,
                    "columns": cols,
                }
            })],
        )
        .await
    }

    pub async fn update_text_style(
        &self,
        presentation_id: &str,
        object_id: &str,
        start_index: i64,
        end_index: i64,
        style: &Value,
        fields: &str,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "updateTextStyle": {
                    "objectId": object_id,
                    "textRange": {
                        "type": "FIXED_RANGE",
                        "startIndex": start_index,
                        "endIndex": end_index,
                    },
                    "style": style,
                    "fields": fields,
                }
            })],
        )
        .await
    }

    pub async fn update_shape_properties(
        &self,
        presentation_id: &str,
        object_id: &str,
        properties: &Value,
        fields: &str,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "updateShapeProperties": {
                    "objectId": object_id,
                    "shapeProperties": properties,
                    "fields": fields,
                }
            })],
        )
        .await
    }

    pub async fn replace_all_shapes_with_image(
        &self,
        presentation_id: &str,
        find_text: &str,
        image_url: &str,
        match_case: bool,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "replaceAllShapesWithImage": {
                    "containsText": {
                        "text": find_text,
                        "matchCase": match_case,
                    },
                    "imageUrl": image_url,
                    "imageReplaceMethod": "CENTER_INSIDE",
                }
            })],
        )
        .await
    }

    pub async fn update_page_properties(
        &self,
        presentation_id: &str,
        page_id: &str,
        properties: &Value,
        fields: &str,
    ) -> Result<Value> {
        self.batch_update(
            presentation_id,
            &[json!({
                "updatePageProperties": {
                    "objectId": page_id,
                    "pageProperties": properties,
                    "fields": fields,
                }
            })],
        )
        .await
    }

    pub async fn create_speaker_notes(
        &self,
        presentation_id: &str,
        slide_id: &str,
        notes_text: &str,
    ) -> Result<Value> {
        // Get the page to find the notes page object ID
        let page = self.get_page(presentation_id, slide_id).await?;
        if let Some(notes_id) = page
            .get("slideProperties")
            .and_then(|sp| sp.get("notesPage"))
            .and_then(|np| np.get("notesProperties"))
            .and_then(|np| np.get("speakerNotesObjectId"))
            .and_then(|id| id.as_str())
        {
            self.insert_text(presentation_id, notes_id, notes_text, 0)
                .await
        } else {
            Err(crate::error::VgoogError::NotFound(
                "Speaker notes object not found".into(),
            ))
        }
    }
}
