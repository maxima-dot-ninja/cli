use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

pub struct DriveApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> DriveApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── Files ──

    pub async fn list_files(
        &self,
        query: Option<&str>,
        page_size: u32,
        page_token: Option<&str>,
        order_by: Option<&str>,
        fields: Option<&str>,
        spaces: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/files?pageSize={page_size}&fields={}",
            urlencoding::encode(fields.unwrap_or("nextPageToken,files(id,name,mimeType,size,modifiedTime,parents,webViewLink,iconLink)"))
        );
        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        if let Some(ob) = order_by {
            url.push_str(&format!("&orderBy={}", urlencoding::encode(ob)));
        }
        if let Some(sp) = spaces {
            url.push_str(&format!("&spaces={sp}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_file(&self, file_id: &str, fields: Option<&str>) -> Result<Value> {
        let f = urlencoding::encode(
            fields.unwrap_or("id,name,mimeType,size,modifiedTime,parents,webViewLink,description,starred,trashed"),
        );
        let url = format!("{BASE}/files/{file_id}?fields={f}");
        self.client.get(&url).await
    }

    pub async fn create_file(&self, metadata: &Value) -> Result<Value> {
        let url = format!("{BASE}/files");
        self.client.post(&url, metadata).await
    }

    pub async fn upload_file(
        &self,
        metadata: &Value,
        content: Vec<u8>,
        mime_type: &str,
    ) -> Result<Value> {
        let url = format!("{UPLOAD_BASE}/files?uploadType=multipart");
        self.client
            .upload_multipart(&url, metadata, content, mime_type)
            .await
    }

    pub async fn update_file_metadata(
        &self,
        file_id: &str,
        metadata: &Value,
    ) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}");
        self.client.patch(&url, metadata).await
    }

    pub async fn update_file_content(
        &self,
        file_id: &str,
        metadata: &Value,
        content: Vec<u8>,
        mime_type: &str,
    ) -> Result<Value> {
        let url = format!("{UPLOAD_BASE}/files/{file_id}?uploadType=multipart");
        self.client
            .upload_multipart(&url, metadata, content, mime_type)
            .await
    }

    pub async fn delete_file(&self, file_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}");
        self.client.delete(&url).await
    }

    pub async fn copy_file(&self, file_id: &str, metadata: &Value) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/copy");
        self.client.post(&url, metadata).await
    }

    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>> {
        let url = format!("{BASE}/files/{file_id}?alt=media");
        self.client.download(&url).await
    }

    pub async fn export_file(&self, file_id: &str, mime_type: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{BASE}/files/{file_id}/export?mimeType={}",
            urlencoding::encode(mime_type)
        );
        self.client.download(&url).await
    }

    pub async fn empty_trash(&self) -> Result<Value> {
        let url = format!("{BASE}/files/trash");
        self.client.delete(&url).await
    }

    pub async fn generate_file_ids(&self, count: u32) -> Result<Value> {
        let url = format!("{BASE}/files/generateIds?count={count}");
        self.client.get(&url).await
    }

    // ── Move file (add/remove parents) ──

    pub async fn move_file(
        &self,
        file_id: &str,
        add_parents: &str,
        remove_parents: &str,
    ) -> Result<Value> {
        let url = format!(
            "{BASE}/files/{file_id}?addParents={}&removeParents={}",
            urlencoding::encode(add_parents),
            urlencoding::encode(remove_parents)
        );
        self.client.patch(&url, &json!({})).await
    }

    // ── Create folder ──

    pub async fn create_folder(&self, name: &str, parent: Option<&str>) -> Result<Value> {
        let mut meta = json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
        });
        if let Some(p) = parent {
            meta["parents"] = json!([p]);
        }
        self.create_file(&meta).await
    }

    // ── Permissions ──

    pub async fn list_permissions(&self, file_id: &str) -> Result<Value> {
        let url = format!(
            "{BASE}/files/{file_id}/permissions?fields=permissions(id,type,role,emailAddress,displayName)"
        );
        self.client.get(&url).await
    }

    pub async fn get_permission(&self, file_id: &str, permission_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/permissions/{permission_id}?fields=*");
        self.client.get(&url).await
    }

    pub async fn create_permission(
        &self,
        file_id: &str,
        role: &str,
        perm_type: &str,
        email: Option<&str>,
    ) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/permissions");
        let mut body = json!({ "role": role, "type": perm_type });
        if let Some(e) = email {
            body["emailAddress"] = json!(e);
        }
        self.client.post(&url, &body).await
    }

    pub async fn update_permission(
        &self,
        file_id: &str,
        permission_id: &str,
        role: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/permissions/{permission_id}");
        self.client.patch(&url, &json!({ "role": role })).await
    }

    pub async fn delete_permission(&self, file_id: &str, permission_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/permissions/{permission_id}");
        self.client.delete(&url).await
    }

    // ── Comments ──

    pub async fn list_comments(&self, file_id: &str, page_token: Option<&str>) -> Result<Value> {
        let mut url = format!("{BASE}/files/{file_id}/comments?fields=comments(id,content,author,createdTime,resolved)");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn create_comment(&self, file_id: &str, content: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/comments?fields=*");
        self.client
            .post(&url, &json!({ "content": content }))
            .await
    }

    pub async fn update_comment(
        &self,
        file_id: &str,
        comment_id: &str,
        content: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/comments/{comment_id}?fields=*");
        self.client
            .patch(&url, &json!({ "content": content }))
            .await
    }

    pub async fn delete_comment(&self, file_id: &str, comment_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/comments/{comment_id}");
        self.client.delete(&url).await
    }

    // ── Replies ──

    pub async fn list_replies(
        &self,
        file_id: &str,
        comment_id: &str,
    ) -> Result<Value> {
        let url = format!(
            "{BASE}/files/{file_id}/comments/{comment_id}/replies?fields=replies(id,content,author,createdTime)"
        );
        self.client.get(&url).await
    }

    pub async fn create_reply(
        &self,
        file_id: &str,
        comment_id: &str,
        content: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/comments/{comment_id}/replies?fields=*");
        self.client
            .post(&url, &json!({ "content": content }))
            .await
    }

    // ── Revisions ──

    pub async fn list_revisions(&self, file_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/revisions?fields=revisions(id,modifiedTime,size,keepForever)");
        self.client.get(&url).await
    }

    pub async fn get_revision(&self, file_id: &str, revision_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/revisions/{revision_id}?fields=*");
        self.client.get(&url).await
    }

    pub async fn delete_revision(&self, file_id: &str, revision_id: &str) -> Result<Value> {
        let url = format!("{BASE}/files/{file_id}/revisions/{revision_id}");
        self.client.delete(&url).await
    }

    // ── Changes ──

    pub async fn get_start_page_token(&self) -> Result<Value> {
        let url = format!("{BASE}/changes/startPageToken");
        self.client.get(&url).await
    }

    pub async fn list_changes(&self, page_token: &str, page_size: u32) -> Result<Value> {
        let url = format!(
            "{BASE}/changes?pageToken={page_token}&pageSize={page_size}&fields=nextPageToken,newStartPageToken,changes(fileId,removed,file(id,name,mimeType))"
        );
        self.client.get(&url).await
    }

    // ── About ──

    pub async fn get_about(&self) -> Result<Value> {
        let url = format!("{BASE}/about?fields=user,storageQuota");
        self.client.get(&url).await
    }

    // ── Shared Drives ──

    pub async fn list_shared_drives(&self, page_token: Option<&str>) -> Result<Value> {
        let mut url = format!("{BASE}/drives?pageSize=100");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn create_shared_drive(&self, name: &str) -> Result<Value> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let url = format!("{BASE}/drives?requestId={request_id}");
        self.client.post(&url, &json!({ "name": name })).await
    }

    pub async fn delete_shared_drive(&self, drive_id: &str) -> Result<Value> {
        let url = format!("{BASE}/drives/{drive_id}");
        self.client.delete(&url).await
    }
}
