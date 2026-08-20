use crate::client::GoogleClient;
use crate::error::Result;
use serde_json::{json, Value};

const BASE: &str = "https://people.googleapis.com/v1";

pub struct PeopleApi<'a> {
    client: &'a GoogleClient,
}

impl<'a> PeopleApi<'a> {
    pub fn new(client: &'a GoogleClient) -> Self {
        Self { client }
    }

    // ── People ──

    pub async fn get_person(&self, resource_name: &str, person_fields: &str) -> Result<Value> {
        let url = format!(
            "{BASE}/{resource_name}?personFields={}", urlencoding::encode(person_fields)
        );
        self.client.get(&url).await
    }

    pub async fn get_me(&self) -> Result<Value> {
        self.get_person(
            "people/me",
            "names,emailAddresses,phoneNumbers,photos,organizations",
        )
        .await
    }

    pub async fn get_batch_people(
        &self,
        resource_names: &[&str],
        person_fields: &str,
    ) -> Result<Value> {
        let mut url = format!("{BASE}/people:batchGet?personFields={}", urlencoding::encode(person_fields));
        for rn in resource_names {
            url.push_str(&format!("&resourceNames={}", urlencoding::encode(rn)));
        }
        self.client.get(&url).await
    }

    // ── Contacts ──

    pub async fn list_contacts(
        &self,
        page_size: u32,
        page_token: Option<&str>,
        person_fields: &str,
        sort_order: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/people/me/connections?pageSize={page_size}&personFields={}",
            urlencoding::encode(person_fields)
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        if let Some(so) = sort_order {
            url.push_str(&format!("&sortOrder={so}"));
        }
        self.client.get(&url).await
    }

    pub async fn search_contacts(&self, query: &str, page_size: u32) -> Result<Value> {
        let url = format!(
            "{BASE}/people:searchContacts?query={}&pageSize={page_size}&readMask=names,emailAddresses,phoneNumbers,organizations",
            urlencoding::encode(query)
        );
        self.client.get(&url).await
    }

    pub async fn create_contact(&self, person: &Value) -> Result<Value> {
        let url = format!("{BASE}/people:createContact");
        self.client.post(&url, person).await
    }

    pub async fn update_contact(
        &self,
        resource_name: &str,
        person: &Value,
        update_mask: &str,
    ) -> Result<Value> {
        let url = format!(
            "{BASE}/{resource_name}:updateContact?updatePersonFields={}",
            urlencoding::encode(update_mask)
        );
        self.client.patch(&url, person).await
    }

    pub async fn delete_contact(&self, resource_name: &str) -> Result<Value> {
        let url = format!("{BASE}/{resource_name}:deleteContact");
        self.client.delete(&url).await
    }

    pub async fn batch_create_contacts(&self, contacts: &[Value]) -> Result<Value> {
        let url = format!("{BASE}/people:batchCreateContacts");
        let body: Vec<Value> = contacts
            .iter()
            .map(|c| json!({ "contactPerson": c }))
            .collect();
        self.client
            .post(&url, &json!({ "contacts": body, "readMask": "names,emailAddresses" }))
            .await
    }

    pub async fn batch_delete_contacts(&self, resource_names: &[&str]) -> Result<Value> {
        let url = format!("{BASE}/people:batchDeleteContacts");
        self.client
            .post(&url, &json!({ "resourceNames": resource_names }))
            .await
    }

    pub async fn batch_update_contacts(
        &self,
        contacts: &Value,
        update_mask: &str,
    ) -> Result<Value> {
        let url = format!("{BASE}/people:batchUpdateContacts");
        self.client
            .post(
                &url,
                &json!({
                    "contacts": contacts,
                    "updateMask": update_mask,
                    "readMask": "names,emailAddresses",
                }),
            )
            .await
    }

    // ── Contact Groups ──

    pub async fn list_contact_groups(&self, page_size: u32, page_token: Option<&str>) -> Result<Value> {
        let mut url = format!("{BASE}/contactGroups?pageSize={page_size}&groupFields=name,memberCount");
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn get_contact_group(&self, resource_name: &str) -> Result<Value> {
        let url = format!("{BASE}/{resource_name}?groupFields=name,memberCount&maxMembers=1000");
        self.client.get(&url).await
    }

    pub async fn create_contact_group(&self, name: &str) -> Result<Value> {
        let url = format!("{BASE}/contactGroups");
        self.client
            .post(
                &url,
                &json!({
                    "contactGroup": { "name": name }
                }),
            )
            .await
    }

    pub async fn update_contact_group(&self, resource_name: &str, name: &str) -> Result<Value> {
        let url = format!("{BASE}/{resource_name}");
        self.client
            .put(
                &url,
                &json!({
                    "contactGroup": { "name": name },
                    "updateGroupFields": "name",
                }),
            )
            .await
    }

    pub async fn delete_contact_group(
        &self,
        resource_name: &str,
        delete_contacts: bool,
    ) -> Result<Value> {
        let url = format!(
            "{BASE}/{resource_name}?deleteContacts={delete_contacts}"
        );
        self.client.delete(&url).await
    }

    pub async fn modify_contact_group_members(
        &self,
        resource_name: &str,
        add: &[&str],
        remove: &[&str],
    ) -> Result<Value> {
        let url = format!("{BASE}/{resource_name}/members:modify");
        self.client
            .post(
                &url,
                &json!({
                    "resourceNamesToAdd": add,
                    "resourceNamesToRemove": remove,
                }),
            )
            .await
    }

    // ── Other Contacts ──

    pub async fn list_other_contacts(
        &self,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/otherContacts?pageSize={page_size}&readMask=names,emailAddresses,phoneNumbers"
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }

    pub async fn copy_other_contact_to_contacts(&self, resource_name: &str) -> Result<Value> {
        let url = format!("{BASE}/{resource_name}:copyOtherContactToMyContactsGroup");
        self.client
            .post(
                &url,
                &json!({
                    "copyMask": "names,emailAddresses,phoneNumbers",
                }),
            )
            .await
    }

    pub async fn search_directory(
        &self,
        query: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<Value> {
        let mut url = format!(
            "{BASE}/people:searchDirectoryPeople?query={}&pageSize={page_size}&readMask=names,emailAddresses,phoneNumbers,organizations&sources=DIRECTORY_SOURCE_TYPE_DOMAIN_PROFILE",
            urlencoding::encode(query)
        );
        if let Some(pt) = page_token {
            url.push_str(&format!("&pageToken={pt}"));
        }
        self.client.get(&url).await
    }
}
