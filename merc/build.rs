//! Turns `openapi.json` into the command table, at compile time.
//!
//! Mercury has 72 operations. Hand-copying them into Rust would drift the moment
//! Mercury ships an endpoint, and nothing would tell us. So the spec is the only
//! source: every path, parameter, type, enum and required flag comes from it, and
//! `tools/fetch-spec.py` refreshes the spec from the live docs.
//!
//! What this file adds on top of the raw spec is the *shape of the CLI*: a group
//! and a verb per operation (`merc cards freeze`), which responses are lists and
//! how they paginate, and which schema each result carries so the printer knows
//! how to lay it out.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;

fn main() {
    println!("cargo:rerun-if-changed=openapi.json");
    println!("cargo:rerun-if-changed=build.rs");

    let root: Value =
        serde_json::from_str(&std::fs::read_to_string("openapi.json").expect("openapi.json is missing"))
            .expect("openapi.json is not valid JSON");
    let spec = Spec { schemas: object(&root["components"]["schemas"]) };

    let mut ops: Vec<Op> = Vec::new();
    for (path, methods) in object(&root["paths"]) {
        for (method, raw) in object(&methods) {
            if !matches!(method.as_str(), "get" | "post" | "put" | "patch" | "delete") {
                continue;
            }
            ops.push(build_op(&spec, &path, &method, &raw));
        }
    }
    ops.sort_by_key(|op| (op.group.clone(), op.verb.clone()));

    reject_collisions(&ops);
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("ops.rs"), render(&ops))
        .unwrap();
}

// ── reading the spec ────────────────────────────────────────────────────────

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

struct Spec {
    schemas: Map<String, Value>,
}

impl Spec {
    /// Follow `$ref` and flatten `allOf`, so callers see one plain object schema.
    ///
    /// Mercury wraps almost every field in `allOf: [{$ref: ...}]` just to hang a
    /// description off it, so without this nearly every type reads as "unknown".
    fn resolve(&self, schema: &Value) -> Value {
        let mut current = schema.clone();
        for _ in 0..8 {
            if let Some(name) = current.get("$ref").and_then(Value::as_str) {
                let target = name.rsplit('/').next().unwrap_or_default();
                current = self.schemas.get(target).cloned().unwrap_or_default();
                continue;
            }
            let Some(members) = current.get("allOf").and_then(Value::as_array).cloned() else {
                break;
            };
            let mut merged = object(&current);
            merged.remove("allOf");
            let mut properties = object(merged.get("properties").unwrap_or(&Value::Null));
            let mut required: Vec<Value> =
                merged.get("required").and_then(Value::as_array).cloned().unwrap_or_default();

            for member in members {
                let member = self.resolve(&member);
                properties.extend(object(member.get("properties").unwrap_or(&Value::Null)));
                required
                    .extend(member.get("required").and_then(Value::as_array).cloned().unwrap_or_default());
                for (key, value) in object(&member) {
                    merged.entry(key).or_insert(value);
                }
            }
            merged.insert("properties".into(), Value::Object(properties));
            merged.insert("required".into(), Value::Array(required));
            current = Value::Object(merged);
        }
        current
    }

    /// The schema's own name (`Transaction`), used to pick a display layout.
    fn name_of(&self, schema: &Value) -> String {
        schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.rsplit('/').next())
            .unwrap_or_default()
            .to_string()
    }
}

// ── one operation ───────────────────────────────────────────────────────────

struct Param {
    name: String,
    loc: &'static str,
    required: bool,
    ty: String,
    choices: Vec<String>,
    about: String,
}

struct Op {
    id: String,
    group: String,
    verb: String,
    about: String,
    notes: String,
    method: String,
    path: String,
    params: Vec<Param>,
    payload: &'static str,
    output: &'static str,
    list_key: Option<String>,
    item_schema: String,
    paging: &'static str,
}

fn build_op(spec: &Spec, path: &str, method: &str, raw: &Value) -> Op {
    let id = text(raw, "operationId");
    let mut params: Vec<Param> = Vec::new();

    for entry in raw["parameters"].as_array().cloned().unwrap_or_default() {
        let schema = spec.resolve(&entry["schema"]);
        let loc = match text(&entry, "in").as_str() {
            "path" => "Path",
            _ => "Query",
        };
        params.push(Param {
            name: text(&entry, "name"),
            loc,
            required: entry["required"].as_bool().unwrap_or(loc == "Path"),
            ty: type_of(&schema),
            choices: choices_of(spec, &schema),
            about: first_line(&[text(&entry, "description"), text(&schema, "description")].join(" ")),
        });
    }

    let body = object(&raw["requestBody"]["content"]);
    let payload = match () {
        _ if body.contains_key("multipart/form-data") => "Multipart",
        _ if body.contains_key("application/x-www-form-urlencoded") => "Form",
        _ if body.contains_key("application/json") => "Json",
        _ => "None",
    };
    let media = ["multipart/form-data", "application/x-www-form-urlencoded", "application/json"]
        .iter()
        .find_map(|kind| body.get(*kind));
    if let Some(media) = media {
        params.extend(body_params(spec, &spec.resolve(&media["schema"])));
    }

    let (output, list_key, item_schema, response_name) = describe_response(spec, raw, method);
    let paging = match () {
        _ if params.iter().any(|p| p.name == "start_after") => "Cursor",
        _ if params.iter().any(|p| p.name == "offset") => "Offset",
        _ => "None",
    };
    let (group, verb) = command_name(&id, &text_of_tag(raw), path);

    Op {
        id,
        group,
        verb,
        about: first_line(&pick_summary(raw)),
        notes: prose(&text(raw, "description")),
        method: method.to_uppercase(),
        path: path.to_string(),
        params,
        payload,
        output,
        list_key,
        item_schema: match item_schema.is_empty() {
            true => response_name,
            false => item_schema,
        },
        paging,
    }
}

fn text_of_tag(raw: &Value) -> String {
    raw["tags"].as_array().and_then(|t| t.first()).and_then(Value::as_str).unwrap_or("misc").to_string()
}

fn pick_summary(raw: &Value) -> String {
    match text(raw, "summary").is_empty() {
        false => text(raw, "summary"),
        true => text(raw, "description"),
    }
}

fn body_params(spec: &Spec, schema: &Value) -> Vec<Param> {
    let properties = object(schema.get("properties").unwrap_or(&Value::Null));
    let required: Vec<String> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // A body that is not an object (rare) is passed through whole as --body.
    if properties.is_empty() {
        return vec![Param {
            name: "body".into(),
            loc: "Body",
            required: true,
            ty: "object".into(),
            choices: vec![],
            about: "Raw JSON request body".into(),
        }];
    }

    properties
        .iter()
        .map(|(name, property)| {
            let property = spec.resolve(property);
            Param {
                name: name.clone(),
                loc: "Body",
                required: required.contains(name),
                ty: type_of(&property),
                choices: choices_of(spec, &property),
                about: first_line(&text(&property, "description")),
            }
        })
        .collect()
}

/// Whether the response is a list, what it is a list of, and how it comes back.
fn describe_response(
    spec: &Spec,
    raw: &Value,
    method: &str,
) -> (&'static str, Option<String>, String, String) {
    let responses = object(&raw["responses"]);
    let Some((_, success)) =
        responses.iter().filter(|(code, _)| code.starts_with('2')).min_by_key(|(code, _)| code.to_string())
    else {
        return ("Json", None, String::new(), String::new());
    };

    let content = object(&success["content"]);
    let Some(json) = content.get("application/json") else {
        // A PDF, or a 204 with nothing in it at all.
        let output = match content.is_empty() {
            true => "Json",
            false => "Binary",
        };
        return (output, None, String::new(), String::new());
    };

    let reference = &json["schema"];
    let response_name = spec.name_of(reference);
    let schema = spec.resolve(reference);

    // A few endpoints answer with the array itself rather than wrapping it —
    // `GET /safes` is the only one today. An empty list key means exactly that.
    if method == "get" && schema["type"] == "array" {
        return ("Json", Some(String::new()), spec.name_of(&schema["items"]), response_name);
    }

    let properties = object(schema.get("properties").unwrap_or(&Value::Null));

    let arrays: Vec<&String> = properties
        .iter()
        .filter(|(_, value)| spec.resolve(value)["type"] == "array")
        .map(|(key, _)| key)
        .collect();

    // A list is a GET whose body is one array, either alone or beside the
    // paging fields. Anything else — a Transaction with an `attachments`
    // array, say — is a single object that happens to contain a list.
    let paged = ["page", "total", "cursor"].iter().any(|key| properties.contains_key(*key));
    let is_list = method == "get" && arrays.len() == 1 && (properties.len() == 1 || paged);
    if !is_list {
        return ("Json", None, String::new(), response_name);
    }

    let key = arrays[0].clone();
    let item = spec.name_of(&properties[&key]["items"]);
    ("Json", Some(key), item, response_name)
}

fn type_of(schema: &Value) -> String {
    if schema.get("enum").is_some() {
        return "enum".into();
    }
    if schema.get("format").and_then(Value::as_str) == Some("binary") {
        return "file".into();
    }
    match text(schema, "type").as_str() {
        "" => "string".into(),
        other => other.into(),
    }
}

/// The allowed values, whether the enum is the parameter itself or the element
/// type of a repeatable one (`status=active&status=frozen`).
fn choices_of(spec: &Spec, schema: &Value) -> Vec<String> {
    let listed = |value: &Value| -> Vec<String> {
        value["enum"]
            .as_array()
            .map(|values| values.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let direct = listed(schema);
    match direct.is_empty() {
        false => direct,
        true => listed(&spec.resolve(&schema["items"])),
    }
}

// ── naming ──────────────────────────────────────────────────────────────────

/// Operations that do not fall out of the rules below cleanly.
const OVERRIDES: &[(&str, &str, &str)] = &[
    ("getOrganization", "organization", "get"),
    ("getSafeRequest", "safes", "get"),
    ("getSafeRequestDocument", "safes", "get-document"),
    ("getSafeRequests", "safes", "list"),
    ("getTransactionById", "transactions", "get"),
    ("revealCardPan", "vault", "reveal"),
    ("startOAuth2Flow", "oauth2", "start-flow"),
];

/// `getAccountStatements` in the Accounts tag becomes `accounts get-statements`.
///
/// The operation id already names the verb and the noun; the tag already names
/// the group. Dropping the group's own words from the id is what turns
/// `freezeCard` into `cards freeze` rather than `cards freeze-card`.
fn command_name(id: &str, tag: &str, path: &str) -> (String, String) {
    if let Some((_, group, verb)) = OVERRIDES.iter().find(|(name, _, _)| *name == id) {
        return (group.to_string(), verb.to_string());
    }

    let group = tag.to_lowercase().replace(' ', "-");
    let mut nouns: Vec<String> = group.split('-').map(String::from).collect();
    nouns.extend(nouns.clone().iter().filter_map(|noun| singular(noun)));

    let words = split_words(id);
    let verb = words[0].to_lowercase();
    let rest: Vec<String> =
        words[1..].iter().map(|word| word.to_lowercase()).filter(|word| !nouns.contains(word)).collect();

    // `getAccounts` reads better as `accounts list`.
    let collection = !path.trim_end_matches('/').ends_with('}');
    let verb = match verb.as_str() {
        "get" if rest.is_empty() && collection => "list".to_string(),
        other => other.to_string(),
    };

    (group, [vec![verb], rest].concat().join("-"))
}

fn singular(word: &str) -> Option<String> {
    match () {
        _ if word.ends_with("ies") => Some(format!("{}y", &word[..word.len() - 3])),
        _ if word.ends_with('s') => Some(word[..word.len() - 1].to_string()),
        _ => None,
    }
}

fn split_words(id: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for character in id.chars() {
        let starts_word = character.is_uppercase() || words.is_empty();
        match starts_word {
            true => words.push(character.to_string()),
            false => words.last_mut().unwrap().push(character),
        }
    }
    words
}

fn reject_collisions(ops: &[Op]) {
    let mut seen: BTreeMap<(String, String), &str> = BTreeMap::new();
    for op in ops {
        let taken = seen.insert((op.group.clone(), op.verb.clone()), &op.id);
        if let Some(other) = taken {
            panic!(
                "`merc {} {}` would run two operations: {} and {}. Add an entry to OVERRIDES in build.rs.",
                op.group, op.verb, other, op.id
            );
        }
    }
}

// ── text tidying ────────────────────────────────────────────────────────────

fn first_line(value: &str) -> String {
    let line = value.trim().lines().next().unwrap_or_default().trim().to_string();
    truncate(&line, 160)
}

/// Mercury's descriptions carry callout blocks and tables that read as noise in a
/// terminal; the prose above them is the part worth showing.
fn prose(description: &str) -> String {
    let kept: Vec<&str> =
        description.lines().take_while(|line| !line.trim_start().starts_with(['>', '|', '#'])).collect();
    truncate(kept.join("\n").trim(), 900)
}

fn truncate(value: &str, limit: usize) -> String {
    match value.chars().count() > limit {
        true => format!("{}…", value.chars().take(limit - 1).collect::<String>()),
        false => value.to_string(),
    }
}

// ── emitting ────────────────────────────────────────────────────────────────

fn render(ops: &[Op]) -> String {
    let mut out = String::from(
        "// Generated by build.rs from openapi.json. Do not edit.\n\npub static OPS: &[Op] = &[\n",
    );
    for op in ops {
        let params: String = op
            .params
            .iter()
            .map(|p| {
                let choices: Vec<String> = p.choices.iter().map(|c| format!("{c:?}")).collect();
                format!(
                    "        Param {{ name: {:?}, loc: In::{}, required: {}, ty: {:?}, choices: &[{}], about: {:?} }},\n",
                    p.name,
                    p.loc,
                    p.required,
                    p.ty,
                    choices.join(", "),
                    p.about
                )
            })
            .collect();

        let _ = write!(
            out,
            "    Op {{\n        id: {:?},\n        group: {:?},\n        verb: {:?},\n        about: {:?},\n        notes: {:?},\n        method: Method::{},\n        path: {:?},\n        payload: Payload::{},\n        output: Output::{},\n        list_key: {},\n        item_schema: {:?},\n        paging: Paging::{},\n        params: &[\n{}        ],\n    }},\n",
            op.id,
            op.group,
            op.verb,
            op.about,
            op.notes,
            match op.method.as_str() {
                "GET" => "Get",
                "POST" => "Post",
                "PATCH" => "Patch",
                "DELETE" => "Delete",
                other => panic!("unhandled method {other}"),
            },
            op.path,
            op.payload,
            op.output,
            match &op.list_key {
                None => "None".to_string(),
                Some(key) => format!("Some({key:?})"),
            },
            op.item_schema,
            op.paging,
            params
        );
    }
    out.push_str("];\n");
    out
}
