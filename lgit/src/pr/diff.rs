//! `lgit pr diff`: a pull request's change, file by file, at one exact head commit.
//!
//! The diff is read as `base...head_sha` rather than "the pull request's diff", so the
//! commit it answers with is the commit the diff shows. Whoever reads it and then merges
//! with `--sha` merges what they read, and a push in between makes the merge refuse.

use super::gh::{Failure, Gh, METHOD_FIELDS};
use super::merge::{readiness, STATE};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct DiffFile {
    pub path: String,
    /// Where it was before a rename or a copy. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_path: Option<String>,
    /// added, deleted, renamed, copied or modified.
    pub status: &'static str,
    pub binary: bool,
    pub additions: u64,
    pub deletions: u64,
    /// The hunks, from the first `@@` on — unified diff text with the file headers taken
    /// off, since `path` and `status` carry them. Empty for a binary file or a mode change.
    pub patch: String,
}

/// One pull request's files at its head commit, and whether it would merge right now.
///
/// The head, the base and the merge state come from one GraphQL call and the diff from
/// a second, at that head, so the two can never disagree. The merge state rides here
/// rather than in the list: it costs 0.6 s for one pull request and 7 s for fifty.
pub async fn fetch(gh: &Gh, owner: &str, name: &str, number: u64) -> Result<Value, Failure> {
    let query = format!(
        "query($o: String!, $n: String!, $num: Int!) {{ repository(owner: $o, name: $n) {{ {METHOD_FIELDS} pullRequest(number: $num) {{ {STATE} }} }} }}"
    );
    let read = gh.graphql(&query, &[("o", owner), ("n", name)], &[("num", number)]).await?;
    let repo = &read["data"]["repository"];
    let pr = &repo["pullRequest"];
    let (Some(sha), Some(base)) = (pr["headRefOid"].as_str(), pr["baseRefName"].as_str()) else {
        return Err(Failure::new("not_found", format!("{owner}/{name} has no pull request #{number}, or this login cannot see it")));
    };
    let files = at(gh, owner, name, base, sha).await?;
    Ok(json!({
        "repo": format!("{owner}/{name}"),
        "number": number,
        "head_sha": sha,
        "base": base,
        "merge": readiness(repo, pr),
        "files": files,
    }))
}

/// The change from where `sha` left `base` to `sha`, which is what GitHub shows as the
/// pull request's files.
pub async fn at(gh: &Gh, owner: &str, name: &str, base: &str, sha: &str) -> Result<Vec<DiffFile>, Failure> {
    let path = format!("repos/{owner}/{name}/compare/{}...{sha}", escape(base));
    let bytes = gh.rest(&path, Some("application/vnd.github.diff")).await?;
    Ok(parse(&String::from_utf8_lossy(&bytes)))
}

/// A branch name as a url path: its slashes stay, anything that would end the path
/// (`#`, `?`, `%`, a space) is percent-encoded.
fn escape(branch: &str) -> String {
    branch
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-' => (byte as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Splits a unified diff at each `diff --git` into its files.
pub fn parse(diff: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut chunk: Vec<&str> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") && !chunk.is_empty() {
            files.push(file_of(&chunk));
            chunk.clear();
        }
        chunk.push(line);
    }
    if !chunk.is_empty() {
        files.push(file_of(&chunk));
    }
    files
}

/// What one file's header lines say, before its first hunk.
#[derive(Default)]
struct Header {
    old: Option<String>,
    new: Option<String>,
    renamed_from: Option<String>,
    renamed_to: Option<String>,
    copied: bool,
    added: bool,
    deleted: bool,
    binary: bool,
}

/// Each header line a diff can carry, and what it sets. Read in order; the first prefix
/// that matches wins.
const HEADERS: &[(&str, fn(&mut Header, &str))] = &[
    ("--- ", |h, rest| h.old = side(rest)),
    ("+++ ", |h, rest| h.new = side(rest)),
    ("rename from ", |h, rest| h.renamed_from = Some(unquote(rest))),
    ("rename to ", |h, rest| h.renamed_to = Some(unquote(rest))),
    ("copy from ", |h, rest| {
        h.renamed_from = Some(unquote(rest));
        h.copied = true;
    }),
    ("copy to ", |h, rest| h.renamed_to = Some(unquote(rest))),
    ("new file mode", |h, _| h.added = true),
    ("deleted file mode", |h, _| h.deleted = true),
    ("Binary files ", |h, _| h.binary = true),
    ("GIT binary patch", |h, _| h.binary = true),
];

fn file_of(lines: &[&str]) -> DiffFile {
    let hunks = lines.iter().position(|line| line.starts_with("@@")).unwrap_or(lines.len());
    let mut header = Header::default();
    for line in &lines[..hunks] {
        if let Some((prefix, set)) = HEADERS.iter().find(|(prefix, _)| line.starts_with(prefix)) {
            set(&mut header, &line[prefix.len()..]);
        }
    }
    let body = &lines[hunks..];
    let count = |mark: char| body.iter().filter(|line| line.starts_with(mark)).count() as u64;
    let named = lines.first().map(|line| git_line_path(line)).unwrap_or_default();
    let path = header.renamed_to.clone().or(header.new.clone()).or(header.old.clone()).unwrap_or(named);
    let marks = [(header.added, "added"), (header.deleted, "deleted"), (header.copied, "copied"), (header.renamed_from.is_some(), "renamed")];
    let status = marks.iter().find(|(set, _)| *set).map(|(_, status)| *status).unwrap_or("modified");
    DiffFile {
        path,
        previous_path: header.renamed_from,
        status,
        binary: header.binary,
        additions: count('+'),
        deletions: count('-'),
        patch: body.join("\n"),
    }
}

/// `a/src/main.rs` → `src/main.rs`; `/dev/null` (the missing side of an add or a
/// delete) → nothing.
fn side(rest: &str) -> Option<String> {
    let path = unquote(rest.split('\t').next().unwrap_or(rest));
    if path == "/dev/null" {
        return None;
    }
    Some(path.strip_prefix("a/").or_else(|| path.strip_prefix("b/")).unwrap_or(&path).to_string())
}

/// The path out of `diff --git a/x b/x`, for a file with no `---`/`+++` lines — a mode
/// change or a binary file.
fn git_line_path(line: &str) -> String {
    let rest = line.trim_start_matches("diff --git ");
    let new = rest.rsplit_once(" b/").map(|(_, path)| path).unwrap_or(rest);
    unquote(new)
}

/// git quotes a path holding unusual characters; the quotes are dropped and its escapes
/// are left as they are.
fn unquote(path: &str) -> String {
    let path = path.trim();
    path.strip_prefix('"').and_then(|inner| inner.strip_suffix('"')).unwrap_or(path).to_string()
}
