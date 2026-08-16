//! The command table: every Mercury operation, generated from `openapi.json`.
//!
//! The table itself is written by `build.rs` and pulled in at the bottom of this
//! file. Everything here is the vocabulary it is written in, plus the lookups the
//! rest of the program does against it.

use anyhow::{bail, Result};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

impl Method {
    pub fn label(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

/// Where a value belongs in the request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum In {
    Path,
    Query,
    Body,
}

/// How the request body is encoded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Payload {
    None,
    Json,
    Form,
    Multipart,
}

/// What comes back: parsed JSON, or bytes to write to a file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output {
    Json,
    Binary,
}

/// How to walk past the first page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paging {
    None,
    /// `start_after=<page.nextPage>`
    Cursor,
    /// `offset=<rows so far>`
    Offset,
}

pub struct Param {
    pub name: &'static str,
    pub loc: In,
    pub required: bool,
    /// string | integer | number | boolean | object | array | enum | file
    pub ty: &'static str,
    pub choices: &'static [&'static str],
    pub about: &'static str,
}

impl Param {
    /// `--start-after` alongside the API's own `--start_after`, because nobody
    /// wants to type camelCase or underscores at a shell prompt.
    pub fn alias(&self) -> Option<String> {
        let kebab = kebab(self.name);
        match kebab == self.name {
            true => None,
            false => Some(kebab),
        }
    }

    pub fn hint(&self) -> String {
        match self.choices.is_empty() {
            false => self.choices.join("|"),
            true => self.ty.to_uppercase(),
        }
    }

    /// Repeatable query parameters accept `a,b` as well as real JSON arrays.
    pub fn is_list(&self) -> bool {
        self.ty == "array"
    }
}

pub struct Op {
    /// Mercury's own operationId, e.g. `getAccountCards`
    pub id: &'static str,
    /// First half of the command, e.g. `accounts`
    pub group: &'static str,
    /// Second half, e.g. `get-cards`
    pub verb: &'static str,
    pub about: &'static str,
    pub notes: &'static str,
    pub method: Method,
    pub path: &'static str,
    pub params: &'static [Param],
    pub payload: Payload,
    pub output: Output,
    /// The response key holding the rows, when the response is a list. An empty
    /// key means the response *is* the array, with nothing wrapped around it —
    /// `GET /safes` answers that way. Use `rows()` rather than reading this.
    pub list_key: Option<&'static str>,
    /// Schema name of one result, which decides how it is displayed
    pub item_schema: &'static str,
    pub paging: Paging,
}

impl Op {
    /// Anything that is not a GET changes something at a bank, so it is confirmed.
    pub fn mutates(&self) -> bool {
        self.method != Method::Get
    }

    pub fn command(&self) -> String {
        format!("{} {}", self.group, self.verb)
    }

    pub fn params_in(&'static self, loc: In) -> impl Iterator<Item = &'static Param> {
        self.params.iter().filter(move |p| p.loc == loc)
    }

    pub fn param(&self, name: &str) -> Option<&'static Param> {
        self.params.iter().find(|p| p.name == name || p.alias().as_deref() == Some(name))
    }

    /// OAuth2 lives on a different host and authenticates as a client, not a user.
    pub fn is_oauth(&self) -> bool {
        self.path.starts_with("/oauth2")
    }

    /// Fill `{placeholders}` from the arguments; the rest stay for query or body.
    pub fn resolve_path(&'static self, args: &Map<String, Value>) -> Result<String> {
        let mut path = self.path.to_string();
        for param in self.params_in(In::Path) {
            let value = args.get(param.name).map(scalar).unwrap_or_default();
            if value.is_empty() {
                bail!("`merc {}` needs --{}", self.command(), param.name);
            }
            path = path.replace(&format!("{{{}}}", param.name), &value);
        }
        Ok(path)
    }

    /// The rows of a listing, wherever this operation happens to keep them.
    pub fn rows<'a>(&self, value: &'a Value) -> Option<&'a Vec<Value>> {
        match self.list_key? {
            "" => value.as_array(),
            key => value[key].as_array(),
        }
    }

    /// What the rows are called, for "12 accounts" and for a spinner.
    pub fn noun(&self) -> &'static str {
        match self.list_key {
            Some("") | None => self.group,
            Some(key) => key,
        }
    }

    /// Named arguments the operation does not have. Catching these here is what
    /// stops a misspelled `--limitt` from silently paging one row at a time.
    pub fn unknown(&self, args: &Map<String, Value>) -> Vec<String> {
        args.keys().filter(|name| *name != RAW_BODY && self.param(name).is_none()).cloned().collect()
    }

    pub fn missing(&self, args: &Map<String, Value>) -> Vec<&'static Param> {
        self.params.iter().filter(|p| p.required && !args.contains_key(p.name)).collect()
    }
}

/// Where a `--body` given whole is kept while it travels with the other
/// arguments. Not a parameter name — no Mercury operation has one starting `__`.
pub const RAW_BODY: &str = "__body";

/// JSON value as a single string, without the quotes a `to_string` would add.
pub fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for character in name.chars() {
        if character.is_uppercase() && !out.is_empty() {
            out.push('-');
        }
        out.push(character.to_ascii_lowercase());
    }
    out.replace('_', "-")
}

pub fn find(group: &str, verb: &str) -> Option<&'static Op> {
    OPS.iter().find(|op| op.group == group && op.verb == verb)
}

/// Accepts `getAccountCards`, `get-account-cards` or `accounts get-cards`.
pub fn find_by_id(name: &str) -> Option<&'static Op> {
    let wanted = kebab(name);
    OPS.iter().find(|op| op.id == name || kebab(op.id) == wanted || op.command() == name.replace('-', " "))
}

pub fn groups() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = OPS.iter().map(|op| op.group).collect();
    names.dedup();
    names
}

pub fn in_group(group: &str) -> Vec<&'static Op> {
    OPS.iter().filter(|op| op.group == group).collect()
}

/// The operation `merc <group>` runs on its own — listing is what people want.
pub fn default_op(group: &str) -> Option<&'static Op> {
    find(group, "list")
}

include!(concat!(env!("OUT_DIR"), "/ops.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn the_whole_api_is_reachable() {
        // 72 operations at the time of vendoring; the guard is against silently
        // losing them, not against Mercury adding more.
        assert!(OPS.len() >= 72, "only {} operations generated", OPS.len());
        for op in OPS {
            assert!(!op.group.is_empty() && !op.verb.is_empty(), "{} has no command name", op.id);
        }
    }

    #[test]
    fn command_names_are_unique() {
        let mut names: Vec<String> = OPS.iter().map(|op| op.command()).collect();
        names.sort();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "two operations share a command name");
    }

    #[test]
    fn the_commands_people_will_type_exist() {
        for (group, verb) in [
            ("accounts", "list"),
            ("accounts", "create-transaction"),
            ("transactions", "list"),
            ("transactions", "get"),
            ("cards", "freeze"),
            ("recipients", "create"),
            ("treasury", "list"),
            ("webhooks", "verify"),
            ("organization", "get"),
            ("safes", "list"),
            ("vault", "reveal"),
        ] {
            assert!(find(group, verb).is_some(), "missing `merc {group} {verb}`");
        }
    }

    #[test]
    fn send_money_knows_what_it_requires() {
        let op = find("accounts", "create-transaction").unwrap();
        let missing: Vec<&str> = op.missing(&Map::new()).iter().map(|p| p.name).collect();
        assert!(missing.contains(&"recipientId"), "got {missing:?}");
        assert!(missing.contains(&"amount"));
        assert!(missing.contains(&"idempotencyKey"));
        assert!(op.mutates());
    }

    #[test]
    fn path_arguments_are_substituted_and_missing_ones_are_named() {
        let op = find("cards", "freeze").unwrap();
        assert_eq!(op.resolve_path(&args(json!({"cardId": "abc"}))).unwrap(), "/cards/abc/freeze");
        let error = op.resolve_path(&Map::new()).unwrap_err().to_string();
        assert!(error.contains("--cardId"), "got: {error}");
    }

    #[test]
    fn lists_are_recognised_and_single_objects_are_not() {
        assert_eq!(find("accounts", "list").unwrap().list_key, Some("accounts"));
        assert_eq!(find("transactions", "list").unwrap().list_key, Some("transactions"));
        assert_eq!(find("merchants", "list").unwrap().list_key, Some("data"));
        // A transaction carries an `attachments` array but is not a list of them.
        assert_eq!(find("transactions", "get").unwrap().list_key, None);
        assert_eq!(find("events", "get").unwrap().list_key, None);
    }

    #[test]
    fn paging_style_comes_from_the_parameters() {
        assert_eq!(find("accounts", "list").unwrap().paging, Paging::Cursor);
        assert_eq!(find("accounts", "list-transactions").unwrap().paging, Paging::Offset);
    }

    #[test]
    fn ids_resolve_however_they_are_written() {
        assert_eq!(find_by_id("getAccountCards").unwrap().id, "getAccountCards");
        assert_eq!(find_by_id("get-account-cards").unwrap().id, "getAccountCards");
        assert!(find_by_id("nonsense").is_none());
    }

    #[test]
    fn flags_get_a_readable_alias() {
        let op = find("accounts", "list").unwrap();
        assert_eq!(op.param("start_after").unwrap().alias().unwrap(), "start-after");
        assert!(op.param("start-after").is_some(), "the alias must resolve too");
    }

    #[test]
    fn typos_are_reported_rather_than_sent() {
        let op = find("accounts", "list").unwrap();
        assert_eq!(op.unknown(&args(json!({"limitt": 5}))), vec!["limitt"]);
        assert!(op.unknown(&args(json!({"limit": 5}))).is_empty());
    }
}
