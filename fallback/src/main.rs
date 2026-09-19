use std::process::ExitCode;

use serde::{Deserialize, Serialize};

const FORBIDDEN: [&str; 4] = ["re-delegation", "coding", "research", "sub_agent"];
const ALLOWED: [&str; 5] = [
    "shell",
    "ingest_read",
    "ingest_list",
    "ingest_grep",
    "ingest_directory",
];

#[derive(Deserialize)]
struct Input {
    task: String,
    failure: Failure,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Failure {
    Timeout,
    Incomplete,
    Error,
}

impl Failure {
    fn describe(self) -> &'static str {
        match self {
            Failure::Timeout => "timed out",
            Failure::Incomplete => "returned incomplete work",
            Failure::Error => "failed with an error",
        }
    }
}

#[derive(Serialize)]
struct Output {
    action: &'static str,
    forbidden: [&'static str; 4],
    allowed: [&'static str; 5],
    rationale: String,
}

fn run() -> Result<String, String> {
    let arg = std::env::args()
        .nth(1)
        .ok_or_else(|| "expected one positional argument: a compact JSON object with fields `task` and `failure`".to_string())?;

    let input: Input = serde_json::from_str(&arg).map_err(|e| format!("invalid input JSON: {e}"))?;

    let rationale = format!(
        "The delegated task {} did not complete because the sub-agent {}. A second delegation would repeat the same failure, so the parent agent must now do the work itself using only read-only ingest tools and the shell; re-delegation, coding, research and spawning another sub_agent are forbidden at this point.",
        quoted(&input.task),
        input.failure.describe()
    );

    let output = Output {
        action: "direct_execution",
        forbidden: FORBIDDEN,
        allowed: ALLOWED,
        rationale,
    };

    serde_json::to_string(&output).map_err(|e| format!("failed to encode output: {e}"))
}

fn quoted(task: &str) -> String {
    format!("\"{}\"", task.replace('"', "'"))
}

fn main() -> ExitCode {
    match run() {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("fallback: {msg}");
            ExitCode::FAILURE
        }
    }
}
