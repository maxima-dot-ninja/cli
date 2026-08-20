use crate::api::people::PeopleApi;
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

fn str_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn val_array(args: &Value, key: &str) -> Vec<Value> {
    args.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub async fn execute(client: &GoogleClient, action: &str, args: Value) -> Result<Value> {
    let api = PeopleApi::new(client);
    match action {
        "get_person" => api.get_person(
            s(&args, "resource_name"),
            so(&args, "person_fields").unwrap_or("names,emailAddresses,phoneNumbers"),
        ).await,
        "get_me" => api.get_me().await,
        "get_batch_people" => {
            let names = str_array(&args, "resource_names");
            let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            api.get_batch_people(
                &name_refs,
                so(&args, "person_fields").unwrap_or("names,emailAddresses"),
            ).await
        }
        "list_contacts" => api.list_contacts(
            u(&args, "page_size", 20), so(&args, "page_token"),
            so(&args, "person_fields").unwrap_or("names,emailAddresses,phoneNumbers"),
            so(&args, "sort_order"),
        ).await,
        "search_contacts" => api.search_contacts(s(&args, "query"), u(&args, "page_size", 10)).await,
        "create_contact" => api.create_contact(&args["person"]).await,
        "update_contact" => api.update_contact(s(&args, "resource_name"), &args["person"], s(&args, "update_mask")).await,
        "delete_contact" => api.delete_contact(s(&args, "resource_name")).await,
        "batch_create_contacts" => {
            let contacts = val_array(&args, "contacts");
            api.batch_create_contacts(&contacts).await
        }
        "batch_delete_contacts" => {
            let names = str_array(&args, "resource_names");
            let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            api.batch_delete_contacts(&name_refs).await
        }
        "batch_update_contacts" => api.batch_update_contacts(&args["contacts"], s(&args, "update_mask")).await,
        "list_contact_groups" => api.list_contact_groups(u(&args, "page_size", 20), so(&args, "page_token")).await,
        "get_contact_group" => api.get_contact_group(s(&args, "resource_name")).await,
        "create_contact_group" => api.create_contact_group(s(&args, "name")).await,
        "update_contact_group" => api.update_contact_group(s(&args, "resource_name"), s(&args, "name")).await,
        "delete_contact_group" => api.delete_contact_group(s(&args, "resource_name"), b(&args, "delete_contacts", false)).await,
        "modify_contact_group_members" => {
            let add = str_array(&args, "add");
            let remove = str_array(&args, "remove");
            let add_refs: Vec<&str> = add.iter().map(|s| s.as_str()).collect();
            let remove_refs: Vec<&str> = remove.iter().map(|s| s.as_str()).collect();
            api.modify_contact_group_members(s(&args, "resource_name"), &add_refs, &remove_refs).await
        }
        "list_other_contacts" => api.list_other_contacts(u(&args, "page_size", 20), so(&args, "page_token")).await,
        "copy_other_contact_to_contacts" => api.copy_other_contact_to_contacts(s(&args, "resource_name")).await,
        "search_directory" => api.search_directory(s(&args, "query"), u(&args, "page_size", 10), so(&args, "page_token")).await,
        _ => Err(VgoogError::Other(format!("Unknown contacts action: {action}"))),
    }
}
