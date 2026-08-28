// Meeting the caller where they are.
//
// Every action here takes snake_case arguments, but the thing calling it — an agent, a script, a
// person — has just been reading Google's documentation, where every field is camelCase. So
// `modify_message` was handed `addLabelIds`, found no `add_labels`, sent Gmail two empty arrays,
// and Gmail replied "no label updates were provided". Nothing was broken; nothing said what was
// wrong either, and the caller concluded the credentials lacked a scope they in fact had.
//
// Normalising here costs one pass over a small object and removes a whole class of that.

use serde_json::{Map, Value};

/// `addLabelIds` → `add_label_ids`. Runs of capitals stay together: `getIMAPSettings` → `get_imap_settings`.
fn to_snake(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let mut out = String::with_capacity(key.len() + 4);

    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let starts_word = i > 0 && (!chars[i - 1].is_ascii_uppercase() || chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase()));
            if starts_word {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            continue;
        }
        out.push(*c);
    }
    out
}

/// Where Google's name and ours differ by more than case. Only for genuinely different words —
/// anything that is merely camelCase is handled above.
const ALIASES: &[(&str, &str)] = &[
    ("add_label_ids", "add_labels"),
    ("remove_label_ids", "remove_labels"),
    ("message_ids", "ids"),
    ("q", "query"),
    ("user_id", "id"),
    ("label_ids", "labels"),
];

/// Accept an argument object however it was spelled.
///
/// Original keys are kept: a caller that got it right is never worse off, and an action reading
/// the exact name still finds it. Added keys never overwrite one that was already there.
pub fn normalize(args: Value) -> Value {
    let Value::Object(original) = &args else { return args };

    let mut merged: Map<String, Value> = original.clone();
    let mut add = |key: String, value: &Value| {
        merged.entry(key).or_insert_with(|| value.clone());
    };

    for (key, value) in original {
        let snake = to_snake(key);
        if snake != *key {
            add(snake.clone(), value);
        }
        if let Some((_, ours)) = ALIASES.iter().find(|(theirs, _)| *theirs == snake) {
            add((*ours).to_string(), value);
        }
    }

    Value::Object(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn google_field_names_reach_our_arguments() {
        // The exact call that failed: Gmail's own spelling, our snake_case reader.
        let args = normalize(json!({ "id": "abc", "addLabelIds": ["Label_140"] }));
        assert_eq!(args["add_labels"], json!(["Label_140"]));
        assert_eq!(args["addLabelIds"], json!(["Label_140"]), "the original must survive");
    }

    #[test]
    fn camel_case_becomes_snake_case() {
        let args = normalize(json!({ "maxResults": 20, "pageToken": "t" }));
        assert_eq!(args["max_results"], json!(20));
        assert_eq!(args["page_token"], json!("t"));
    }

    #[test]
    fn a_correct_call_is_left_alone() {
        let args = normalize(json!({ "add_labels": ["A"] }));
        assert_eq!(args["add_labels"], json!(["A"]));
    }

    #[test]
    fn runs_of_capitals_stay_together() {
        assert_eq!(to_snake("getIMAPSettings"), "get_imap_settings");
        assert_eq!(to_snake("addLabelIds"), "add_label_ids");
    }
}
