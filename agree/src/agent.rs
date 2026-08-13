use crate::ai::{dispatch, Intent};
use crate::client::Client;
use crate::config::Config;
use crate::tools;
use anyhow::Result;
use console::style;
use dialoguer::{Confirm, Input};
use serde::Deserialize;
use serde_json::{Map, Value};

/// How many tool calls before we stop. Enough for "find the contact, list their
/// invoices, work out the gaps"; short enough that a confused model cannot spin.
const MAX_STEPS: usize = 10;

/// Tool results are fed back as text. A whole page of invoices is far more than
/// the model needs and crowds out its own reasoning, so results get trimmed.
const MAX_RESULT_CHARS: usize = 6000;

/// A conversation keeps its history so follow-ups make sense, but an unbounded
/// transcript eventually costs more than it is worth — keep the recent tail.
const MAX_TRANSCRIPT_CHARS: usize = 40_000;

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
- Never invent an id, an email address, or an amount. If you need one and cannot find it, use the answer shape to ask the user for it.
- Tools marked [CHANGES DATA] are shown to the user for confirmation before they run. Call them only when the user clearly asked for that change.
- When you have enough to answer, answer. Do not keep calling tools to be thorough.

DIAGNOSE, DO NOT JUST REPORT.
You are looking at someone's billing. A plain readout of what exists is not useful if something is wrong with it. Notice problems, say them plainly, and propose a fix.

Always do the arithmetic yourself. You know today's date. Count the periods.

For a recurring series, work out all three of these before answering:
1. How many occurrences SHOULD exist by today — count the repeat periods from the first due date up to today. A weekly series first due 4 weeks ago should have produced about 5 invoices (the first, plus one per week since).
2. How many actually exist — count what the API returned, and look at the highest recurring_sequence.
3. The gap between them. If fewer exist than should, that is the headline of your answer, not a footnote.

"recurring_end_type": "never" means the series has no END date. It does NOT mean the number issued so far is unknowable — that is always countable from the start date to today. Never answer "indeterminate" to "how many should have been created"; count them.

Other things worth flagging when you see them:
- scheduled_at in the past — the invoice was backdated and may have sent at an unexpected time
- a series stuck at recurring_sequence 0 long after its start date — it is not generating
- invoices long overdue, or a due date earlier than the send date

When you flag a problem, end with what to do about it: the specific tool that would fix it, or the specific thing the user should change.

- Today is {}. Use this date for every calculation about what is late, overdue, or missing."#,
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

/// Answer one question, then keep the prompt open for follow-ups. The transcript
/// carries across turns, so "yes, do that" refers to what was just discussed.
pub async fn converse(config: &Config, api: &Client, first: Option<String>) -> Result<()> {
    let system = system_prompt();
    let mut transcript = String::new();
    let mut pending = first;

    println!("  {}", style("Ask anything. Enter on an empty line to leave.").dim());

    loop {
        let question = match pending.take() {
            Some(text) => {
                println!("\n  {} {}", style("›").cyan().bold(), style(&text).bold());
                text
            }
            None => match read_question()? {
                Some(text) => text,
                None => break,
            },
        };

        transcript.push_str(&format!("\nUser asked: {question}\n"));
        answer(config, api, &system, &mut transcript).await?;
        trim_transcript(&mut transcript);
    }

    println!("  {}\n", style("Bye.").dim());
    Ok(())
}

fn read_question() -> Result<Option<String>> {
    println!();
    let text: String = Input::new()
        .with_prompt("›")
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    let text = text.trim().to_string();
    let leaving = text.is_empty()
        || ["exit", "quit", "q", ":q", "bye"].contains(&text.to_lowercase().as_str());
    Ok((!leaving).then_some(text))
}

/// Keep the tail. Cutting from the front loses the oldest turns first, which are
/// the ones a follow-up is least likely to be about.
fn trim_transcript(transcript: &mut String) {
    if transcript.len() <= MAX_TRANSCRIPT_CHARS {
        return;
    }
    let start = transcript.len() - MAX_TRANSCRIPT_CHARS;
    let boundary = transcript[start..].find('\n').map(|i| start + i).unwrap_or(start);
    *transcript = format!("(earlier conversation trimmed)\n{}", &transcript[boundary..]);
}

async fn answer(
    config: &Config,
    api: &Client,
    system: &str,
    transcript: &mut String,
) -> Result<()> {
    for step_number in 1..=MAX_STEPS {
        let spinner = crate::ui::spinner("Thinking...");
        let raw = dispatch(&config.ai, system, transcript).await;
        spinner.finish_and_clear();

        let raw = raw?;
        if std::env::var("AGREE_DEBUG").is_ok() {
            println!("{}", style(format!("  [raw] {raw}")).dim());
        }

        let Some(step) = parse_step(&raw) else {
            println!("  {}\n", style("The model returned nothing.").yellow());
            return Ok(());
        };

        if let Some(reply) = step.answer {
            println!("\n{}", indent(&reply));
            transcript.push_str(&format!("\nYou answered: {reply}\n"));
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
        if tool.mutates && name != "create_invoice" && !confirm(tool, &args)? {
            println!("  {}\n", style("Skipped.").dim());
            transcript.push_str("\nThe user declined that change. Do not retry it.\n");
            continue;
        }

        // Creating an invoice runs the form instead of a raw POST, so the payee
        // is resolved, dollars become cents and the repeat gets its weekday.
        let outcome = match name.as_str() {
            "create_invoice" => create_via_form(api, config, &args).await,
            _ => {
                let spinner = crate::ui::spinner(&format!("{name}..."));
                let result = tools::run(api, tool, &args).await;
                spinner.finish_and_clear();
                result
            }
        };

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

/// Build an invoice through the interactive form, using whatever the model worked
/// out as the starting point.
async fn create_via_form(api: &Client, config: &Config, args: &Map<String, Value>) -> Result<Value> {
    let text = |key: &str| args.get(key).and_then(|v| v.as_str()).map(String::from);

    let intent = Intent {
        action: "create_invoice".into(),
        amount: text("amount"),
        payee: text("payee"),
        repeat_unit: text("repeat_unit"),
        repeat_frequency: args.get("repeat_frequency").and_then(|v| v.as_i64()),
        due_date: text("due_date"),
        memo: text("memo"),
        ..Default::default()
    };

    println!();
    let Some(invoice) = crate::forms::build_invoice(api, &intent, &config.currency).await? else {
        return Ok(serde_json::json!({"cancelled": "the user declined to create the invoice"}));
    };

    let spinner = crate::ui::spinner("Creating invoice...");
    let result = api.post_raw("/api/v1/invoices", &serde_json::json!({"invoice": invoice})).await;
    spinner.finish_and_clear();
    result
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
    fn the_prompt_carries_todays_date() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let prompt = system_prompt();
        assert!(prompt.contains(&today), "the agent must know what day it is");
        assert!(prompt.contains("Use this date for every calculation"));
    }

    #[test]
    fn the_prompt_demands_diagnosis_not_a_readout() {
        let prompt = system_prompt();
        assert!(prompt.contains("DIAGNOSE, DO NOT JUST REPORT"));
        assert!(prompt.contains("Never answer \"indeterminate\""));
        assert!(prompt.contains("Count the periods"));
    }

    #[test]
    fn the_prompt_lists_every_tool() {
        let prompt = system_prompt();
        for tool in tools::TOOLS {
            assert!(prompt.contains(tool.name), "{} missing from prompt", tool.name);
        }
    }
}
