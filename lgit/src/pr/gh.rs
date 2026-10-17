//! Every call `lgit pr` makes goes through `gh`, so it reads and merges as whoever
//! `gh` is logged in as. Under Vaulty's launchd that is the owner — the same login he
//! gets when he types `gh` himself.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Why a command did not answer what was asked. `reason` is a short code a program
/// can branch on; `error` is the sentence a person reads.
#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    pub reason: &'static str,
    pub error: String,
}

impl Failure {
    pub fn new(reason: &'static str, error: impl Into<String>) -> Self {
        Self { reason, error: error.into() }
    }
}

/// `gh`, and the moment the whole command has to be done by. Every call gets what is
/// left of that budget, so three calls in a row can never add up past it.
#[derive(Clone)]
pub struct Gh {
    bin: PathBuf,
    deadline: Instant,
}

/// Where `gh` lives when PATH is launchd's minimal one: the same places Vaulty's
/// `find_binary` searches.
const PLACES: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

impl Gh {
    pub fn find(timeout: Duration) -> Result<Self, Failure> {
        let path = std::env::var("PATH").unwrap_or_default();
        let home = dirs::home_dir().unwrap_or_default();
        let found = path
            .split(':')
            .map(PathBuf::from)
            .chain(PLACES.iter().map(PathBuf::from))
            .chain([home.join(".local/bin")])
            .map(|dir| dir.join("gh"))
            .find(|candidate| candidate.is_file());
        let Some(bin) = found else { return Err(Failure::new("no_gh", "gh is not installed")) };
        Ok(Self { bin, deadline: Instant::now() + timeout })
    }

    /// Runs `gh` with these arguments and hands back whether it exited 0, its stdout, and
    /// the first line of its stderr. A call still running at the deadline is killed.
    async fn run(&self, args: &[String]) -> Result<(bool, Vec<u8>, String), Failure> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        let run = tokio::process::Command::new(&self.bin)
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output();
        let Ok(done) = tokio::time::timeout(left, run).await else {
            return Err(Failure::new("timed_out", "GitHub did not answer in time"));
        };
        let output = done.map_err(|problem| Failure::new("gh_failed", format!("could not run gh: {problem}")))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default().to_string();
        Ok((output.status.success(), output.stdout, first))
    }

    /// One GraphQL call. Strings go in with `-f` and numbers with `-F`, so nothing is ever
    /// spliced into the query text. `gh` exits 1 when any field failed and still prints
    /// the rest, so the answer is read whatever the exit code, and only an answer with no
    /// `data` at all is a failure.
    pub async fn graphql(&self, query: &str, strings: &[(&str, &str)], numbers: &[(&str, u64)]) -> Result<Value, Failure> {
        let mut args = vec!["api".to_string(), "graphql".into(), "-f".into(), format!("query={query}")];
        for (name, value) in strings {
            args.extend(["-f".into(), format!("{name}={value}")]);
        }
        for (name, value) in numbers {
            args.extend(["-F".into(), format!("{name}={value}")]);
        }
        let (_, stdout, stderr) = self.run(&args).await?;
        let answer: Value = serde_json::from_slice(&stdout).unwrap_or(Value::Null);
        if answer["data"].is_object() {
            return Ok(answer);
        }
        let said = answer["errors"][0]["message"].as_str().map(str::to_string);
        Err(Failure::new("gh_failed", said.unwrap_or_else(|| or(stderr, "gh gave no answer"))))
    }

    /// One REST call, answered as raw bytes. `accept` asks for another media type — the
    /// diff rather than JSON. A 404 is its own reason, since it means "no such pull
    /// request" rather than "GitHub is down".
    pub async fn rest(&self, path: &str, accept: Option<&str>) -> Result<Vec<u8>, Failure> {
        let mut args = vec!["api".to_string(), path.to_string()];
        if let Some(accept) = accept {
            args.extend(["-H".into(), format!("Accept: {accept}")]);
        }
        let (ok, stdout, stderr) = self.run(&args).await?;
        if ok {
            return Ok(stdout);
        }
        if stderr.contains("HTTP 404") {
            return Err(Failure::new("not_found", "GitHub has no such pull request, or this login cannot see it"));
        }
        let said: Value = serde_json::from_slice(&stdout).unwrap_or(Value::Null);
        let message = said["message"].as_str().map(str::to_string);
        Err(Failure::new("gh_failed", message.unwrap_or_else(|| or(stderr, "gh gave no answer"))))
    }
}

fn or(text: String, fallback: &str) -> String {
    if text.is_empty() {
        return fallback.to_string();
    }
    text
}

/// `(owner, name)` from `owner/name` or any GitHub url an `origin` takes.
pub fn repo_of(text: &str) -> Option<(String, String)> {
    let text = text.trim().trim_end_matches('/');
    let text = text.strip_suffix(".git").unwrap_or(text);
    let starts = ["https://github.com/", "http://github.com/", "ssh://git@github.com/", "git@github.com:", "git://github.com/"];
    let path = starts.iter().find_map(|start| text.strip_prefix(start)).unwrap_or(text);
    let (owner, name) = path.split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some((owner.to_string(), name.to_string()))
}

/// How a repo merges when nothing says otherwise: squash when it allows squash, else
/// the method GitHub would offer this login first, else whichever one it allows.
pub fn default_method(repo: &Value) -> Option<&'static str> {
    let allowed = |field: &str| repo[field].as_bool().unwrap_or(false);
    if allowed("squashMergeAllowed") {
        return Some("squash");
    }
    let methods = [("merge", "mergeCommitAllowed"), ("rebase", "rebaseMergeAllowed")];
    let preferred = repo["viewerDefaultMergeMethod"].as_str().unwrap_or_default().to_lowercase();
    let pick = methods.iter().find(|(method, field)| *method == preferred && allowed(field));
    pick.or_else(|| methods.iter().find(|(_, field)| allowed(field))).map(|(method, _)| *method)
}

/// The repo fields `default_method` reads, for any query that needs them.
pub const METHOD_FIELDS: &str = "squashMergeAllowed mergeCommitAllowed rebaseMergeAllowed viewerDefaultMergeMethod";
