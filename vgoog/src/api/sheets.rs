use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://sheets.googleapis.com/v4/spreadsheets";

pub struct SheetsApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> SheetsApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Spreadsheets ──

    pub async fn create_spreadsheet(&self, title: &str) -> Result<Value> {
        let url = BASE.to_string();
        self.client
            .post(
                &url,
                &json!({
                    "properties": { "title": title }
                }),
            )
            .await
    }

    pub async fn get_spreadsheet(&self, id: &str) -> Result<Value> {
        let url = format!("{BASE}/{id}");
        self.client.get(&url).await
    }

    pub async fn get_spreadsheet_with_ranges(
        &self,
        id: &str,
        ranges: &[&str],
    ) -> Result<Value> {
        let mut url = format!("{BASE}/{id}?includeGridData=true");
        for r in ranges {
            url.push_str(&format!("&ranges={}", urlencoding::encode(r)));
        }
        self.client.get(&url).await
    }

    // ── Values ──

    pub async fn get_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
        value_render: Option<&str>,
    ) -> Result<Value> {
        let r = urlencoding::encode(range);
        let mut url = format!("{BASE}/{spreadsheet_id}/values/{r}");
        if let Some(vr) = value_render {
            url.push_str(&format!("?valueRenderOption={vr}"));
        }
        self.client.get(&url).await
    }

    pub async fn batch_get_values(
        &self,
        spreadsheet_id: &str,
        ranges: &[&str],
    ) -> Result<Value> {
        let mut url = format!("{BASE}/{spreadsheet_id}/values:batchGet?");
        for (i, r) in ranges.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(&format!("ranges={}", urlencoding::encode(r)));
        }
        self.client.get(&url).await
    }

    pub async fn update_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
        values: &Value,
        input_option: &str,
    ) -> Result<Value> {
        let r = urlencoding::encode(range);
        let url = format!(
            "{BASE}/{spreadsheet_id}/values/{r}?valueInputOption={input_option}"
        );
        self.client
            .put(
                &url,
                &json!({
                    "range": range,
                    "values": values,
                }),
            )
            .await
    }

    pub async fn append_values(
        &self,
        spreadsheet_id: &str,
        range: &str,
        values: &Value,
        input_option: &str,
    ) -> Result<Value> {
        let r = urlencoding::encode(range);
        let url = format!(
            "{BASE}/{spreadsheet_id}/values/{r}:append?valueInputOption={input_option}&insertDataOption=INSERT_ROWS"
        );
        self.client
            .post(
                &url,
                &json!({
                    "range": range,
                    "values": values,
                }),
            )
            .await
    }

    pub async fn clear_values(&self, spreadsheet_id: &str, range: &str) -> Result<Value> {
        let r = urlencoding::encode(range);
        let url = format!("{BASE}/{spreadsheet_id}/values/{r}:clear");
        self.client.post(&url, &json!({})).await
    }

    pub async fn batch_update_values(
        &self,
        spreadsheet_id: &str,
        data: &[Value],
        input_option: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/{spreadsheet_id}/values:batchUpdate");
        self.client
            .post(
                &url,
                &json!({
                    "valueInputOption": input_option,
                    "data": data,
                }),
            )
            .await
    }

    pub async fn batch_clear_values(
        &self,
        spreadsheet_id: &str,
        ranges: &[&str],
    ) -> Result<Value> {
        let url = format!("{BASE}/{spreadsheet_id}/values:batchClear");
        self.client
            .post(&url, &json!({ "ranges": ranges }))
            .await
    }

    // ── Batch Update (structural) ──

    pub async fn batch_update(
        &self,
        spreadsheet_id: &str,
        requests: &[Value],
    ) -> Result<Value> {
        let url = format!("{BASE}/{spreadsheet_id}:batchUpdate");
        self.client
            .post(&url, &json!({ "requests": requests }))
            .await
    }

    // ── Convenience: Add Sheet ──

    pub async fn add_sheet(&self, spreadsheet_id: &str, title: &str) -> Result<Value> {
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "addSheet": {
                    "properties": { "title": title }
                }
            })],
        )
        .await
    }

    pub async fn delete_sheet(&self, spreadsheet_id: &str, sheet_id: i64) -> Result<Value> {
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "deleteSheet": {
                    "sheetId": sheet_id
                }
            })],
        )
        .await
    }

    pub async fn rename_sheet(
        &self,
        spreadsheet_id: &str,
        sheet_id: i64,
        new_title: &str,
    ) -> Result<Value> {
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "updateSheetProperties": {
                    "properties": {
                        "sheetId": sheet_id,
                        "title": new_title,
                    },
                    "fields": "title",
                }
            })],
        )
        .await
    }

    pub async fn auto_resize_columns(
        &self,
        spreadsheet_id: &str,
        sheet_id: i64,
        start_col: i64,
        end_col: i64,
    ) -> Result<Value> {
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "autoResizeDimensions": {
                    "dimensions": {
                        "sheetId": sheet_id,
                        "dimension": "COLUMNS",
                        "startIndex": start_col,
                        "endIndex": end_col,
                    }
                }
            })],
        )
        .await
    }

    pub async fn sort_range(
        &self,
        spreadsheet_id: &str,
        sheet_id: i64,
        start_row: i64,
        end_row: i64,
        start_col: i64,
        end_col: i64,
        sort_col: i64,
        ascending: bool,
    ) -> Result<Value> {
        let order = if ascending { "ASCENDING" } else { "DESCENDING" };
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "sortRange": {
                    "range": {
                        "sheetId": sheet_id,
                        "startRowIndex": start_row,
                        "endRowIndex": end_row,
                        "startColumnIndex": start_col,
                        "endColumnIndex": end_col,
                    },
                    "sortSpecs": [{
                        "dimensionIndex": sort_col,
                        "sortOrder": order,
                    }]
                }
            })],
        )
        .await
    }

    pub async fn create_named_range(
        &self,
        spreadsheet_id: &str,
        name: &str,
        sheet_id: i64,
        start_row: i64,
        end_row: i64,
        start_col: i64,
        end_col: i64,
    ) -> Result<Value> {
        self.batch_update(
            spreadsheet_id,
            &[json!({
                "addNamedRange": {
                    "namedRange": {
                        "name": name,
                        "range": {
                            "sheetId": sheet_id,
                            "startRowIndex": start_row,
                            "endRowIndex": end_row,
                            "startColumnIndex": start_col,
                            "endColumnIndex": end_col,
                        }
                    }
                }
            })],
        )
        .await
    }
}
