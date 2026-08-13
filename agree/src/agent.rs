use crate::ai::dispatch;
use crate::client::Client;
use crate::config::Config;
use crate::tools;
use anyhow::Result;
use console::style;
use dialoguer::Confirm;
use serde::Deserialize;
use serde_json::{Map, Value};

/// How many tool calls before we stop. Enough for "find the contact, list their
/// invoices, work out the gaps"; short enough that a confused model cannot spin.
const MAX_STEPS: usize = 10;

/// Tool results are fed back as text. A whole page of invoices is far more than
/// the model needs and crowds out its own reasoning, so results get trimmed.
const MAX_RESULT_CHARS: usize = 6000;

fn system_prompt() -> String {
    format!(
        r#"You are the agent behind a command line tool for the Agree invoicing API. You answer questions and make changes by calling tools, one at a time.

Reply with raw JSON only — no prose outside it, no markdown, no code fences. Exactly one of these two shapes:

To call a tool:
{{"tool": "tool_name", "args": {{...}}, "why": "short reason, shown to the user"}}

To finish:
{{"answer": "your reply to the user, in plain language"}}

Tools available:

{}

How to work:
- Take one step at a time. You will be shown each result before deciding the next step.
- The API cannot search contacts by person name. To find "Samir", call list_contacts and read the names in the result yourself, then use that contact's id.
- Money is ALWAYS in integer cents. 500000 means $5,000.00. When reporting amounts to the user, convert to dollars.
- Invoices in a repeating series share recurring_options and count up through recurring_sequence. To reason about a weekly series, compare the dates you can see against the schedule in recurring_options.
- Never invent an id, an email address, or an amount. If you need one and cannot find it, use the answer shape to ask the user for it.
- Tools marked [CHANGES DATA] are shown to the user for confirmation before they run. Call them only when the user clearly asked for that change.
- When you have enough to answer, answer. Do not keep calling tools to be thorough.
- Today is {}."#,
        tools::catalogue(),
        chrono::Local::now().format("%Y-%m-%d (%A)")
    )
}

#[derive(Deserialize)]
struct Step {
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    args: Option<Value>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    answer: Option<String>,
}

/// Pull the JSON object out of a reply, tolerating fences and stray prose.
///
/// Models drift out of the format once a few tool results are in the transcript
/// and just start writing the answer. That reply is still the answer, so plain
/// prose is taken as one rather than thrown away.
fn parse_step(raw: &str) -> Option<Step> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }

    let object = text.find('{').zip(text.rfind('}')).filter(|(s, e)| e > s);
    match object.and_then(|(start, end)| serde_json::from_str::<Step>(&text[start..=end]).ok()) {
        Some(step) => Some(step),
        None => Some(Step {
            tool: None,
            args: None,
            why: None,
            answer: Some(text.to_string()),
        }),
    }
}

fn trim_result(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_default();
    match text.len() > MAX_RESULT_CHARS {
        true => format!("{}… (truncated)", &text[..MAX_RESULT_CHARS]),
        false => text,
    }
}

fn as_args(value: Option<Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

pub async fn run(config: &Config, api: &Client, request: &str) -> Result<()> {
    let system = system_prompt();
    let mut transcript = format!("User asked: {request}\n");

    for step_number in 1..=MAX_STEPS {
        let spinner = crate::ui::spinner("Thinking...");
        let raw = dispatch(&config.ai, &system, &transcript).await;
        spinner.finish_and_clear();

        let raw = raw?;
        if std::env::var("AGREE_DEBUG").is_ok() {
            println!("{}", style(format!("  [raw] {raw}")).dim());
        }

        let Some(step) = parse_step(&raw) else {
            println!("  {}\n", style("The model returned nothing.").yellow());
            return Ok(());
        };

        if let Some(answer) = step.answer {
            println!("\n{}\n", indent(&answer));
            return Ok(());
        }

        let Some(name) = step.tool else {
            println!("  {}", style("The model neither answered nor called a tool.").yellow());
            return Ok(());
        };

        let Some(tool) = tools::find(&name) else {
            transcript.push_str(&format!("\nTool `{name}` does not exist. Use one from the list.\n"));
            continue;
        };

        let args = as_args(step.args);

        if let Some(why) = &step.why {
            println!("  {} {}", style("→").cyan(), style(why).dim());
        }

        // Anything that changes data stops here until the user says go.
        if tool.mutates && !confirm(tool, &args)? {
            println!("  {}\n", style("Skipped.").dim());
            transcript.push_str("\nThe user declined that change. Do not retry it.\n");
            continue;
        }

        let spinner = crate::ui::spinner(&format!("{name}..."));
        let outcome = tools::run(api, tool, &args).await;
        spinner.finish_and_clear();

        let summary = match outcome {
            Ok(value) => {
                if tool.mutates {
                    crate::ui::success(&format!("{name} done"));
                }
                trim_result(&value)
            }
            // A failure is information for the model, not the end of the run — it
            // can correct a bad id or a missing field and try again.
            Err(e) => format!("that call failed: {e}"),
        };

        transcript.push_str(&format!(
            "\nStep {step_number}: called {name} with {}\nResult: {summary}\n\nReply with the next tool call as JSON, or {{\"answer\": \"...\"}} if you can now answer.\n",
            serde_json::to_string(&Value::Object(args)).unwrap_or_default()
        ));
    }

    println!(
        "  {}\n",
        style(format!("Stopped after {MAX_STEPS} steps without a final answer.")).yellow()
    );
    Ok(())
}

fn confirm(tool: &tools::Tool, args: &Map<String, Value>) -> Result<bool> {
    println!();
    println!("{}", style("  ── This will change your data ──").yellow());
    for line in tools::describe_call(tool, args).lines() {
        println!("  {line}");
    }
    println!();
    Ok(Confirm::new().with_prompt("Go ahead?").default(false).interact()?)
}

fn indent(text: &str) -> String {
    text.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_tool_call() {
        let step = parse_step(r#"{"tool":"list_contacts","args":{"page_size":100},"why":"find Samir"}"#).unwrap();
        assert_eq!(step.tool.unwrap(), "list_contacts");
        assert_eq!(step.why.unwrap(), "find Samir");
    }

    #[test]
    fn reads_a_final_answer() {
        let step = parse_step(r#"{"answer":"You have sent 3 invoices."}"#).unwrap();
        assert_eq!(step.answer.unwrap(), "You have sent 3 invoices.");
        assert!(step.tool.is_none());
    }

    #[test]
    fn survives_fences_and_chatter() {
        let step = parse_step("Sure:\n```json\n{\"tool\":\"list_invoices\",\"args\":{}}\n```").unwrap();
        assert_eq!(step.tool.unwrap(), "list_invoices");
    }

    #[test]
    fn missing_args_become_an_empty_map() {
        let step = parse_step(r#"{"tool":"list_templates"}"#).unwrap();
        assert!(as_args(step.args).is_empty());
    }

    #[test]
    fn prose_is_treated_as_the_answer() {
        // Observed live: after four tool calls the model wrote its conclusion as
        // markdown instead of JSON. That reply is the answer, not a failure.
        let step = parse_step("Based on what I found:\n\n**Samir** has 1 invoice.").unwrap();
        assert!(step.tool.is_none());
        assert!(step.answer.unwrap().contains("1 invoice"));
    }

    #[test]
    fn json_still_wins_over_prose() {
        let step = parse_step("Here you go:\n{\"tool\":\"list_invoices\",\"args\":{}}").unwrap();
        assert_eq!(step.tool.unwrap(), "list_invoices");
        assert!(step.answer.is_none());
    }

    #[test]
    fn an_empty_reply_is_still_nothing() {
        assert!(parse_step("   ").is_none());
    }

    #[test]
    fn long_results_are_trimmed_not_dropped() {
        let big = Value::String("x".repeat(MAX_RESULT_CHARS * 2));
        let out = trim_result(&big);
        assert!(out.len() < MAX_RESULT_CHARS + 100);
        assert!(out.ends_with("… (truncated)"));
    }

    #[test]
    fn the_prompt_lists_every_tool() {
        let prompt = system_prompt();
        for tool in tools::TOOLS {
            assert!(prompt.contains(tool.name), "{} missing from prompt", tool.name);
        }
    }
}
