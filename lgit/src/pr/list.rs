//! `lgit pr list`: the open pull requests on each repo asked about, in one GraphQL
//! call however many repos there are — six repos cost one round trip, not six.

use super::diff;
use super::gh::{default_method, repo_of, Failure, Gh, METHOD_FIELDS};
use serde_json::{json, Value};
use std::sync::Arc;

/// Read per repo, newest activity first.
///
/// Whether each one can merge is NOT read here. `mergeStateStatus`, the checks rollup and
/// `reviewDecision` took this call from 1.0 s to 8.1 s on cli/cli's 50 open pull
/// requests (3.8 s, 2.9 s and 1.75 s of it), and only the Merge button reads them — so
/// `lgit pr diff` asks for them, for the one pull request on screen, in 0.6 s.
const PULLS: &str = "pullRequests(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { \
    number title url body isDraft createdAt updatedAt baseRefName headRefName headRefOid \
    additions deletions changedFiles author { login avatarUrl } labels(first: 20) { nodes { name color } } } }";

/// How many diffs are fetched at once with `--diffs`.
const DIFFS_AT_ONCE: usize = 6;

pub async fn run(gh: &Gh, repos: &[String], diffs: bool) -> Result<Value, Failure> {
    let mut asked: Vec<(String, String)> = Vec::new();
    for text in repos {
        let Some(repo) = repo_of(text) else { return Err(Failure::new("bad_repo", format!("{text} is not owner/name or a GitHub url"))) };
        if !asked.contains(&repo) {
            asked.push(repo);
        }
    }
    if asked.is_empty() {
        return Ok(json!({ "repos": [] }));
    }
    let answer = ask(gh, &asked).await?;
    let mut out: Vec<Value> = asked.iter().enumerate().map(|(index, (owner, name))| read_repo(&answer, index, owner, name)).collect();
    if diffs {
        attach_diffs(gh, &mut out).await;
    }
    Ok(json!({ "repos": out }))
}

/// One `repository` field per repo, aliased `r0`, `r1`, …, with owner and name as variables.
async fn ask(gh: &Gh, repos: &[(String, String)]) -> Result<Value, Failure> {
    let variables: Vec<String> = (0..repos.len()).map(|i| format!("$o{i}: String!, $n{i}: String!")).collect();
    let fields: Vec<String> = (0..repos.len()).map(|i| format!("r{i}: repository(owner: $o{i}, name: $n{i}) {{ ...repo }}")).collect();
    let query = format!(
        "query({}) {{ {} }} fragment repo on Repository {{ url defaultBranchRef {{ name }} {METHOD_FIELDS} {PULLS} }}",
        variables.join(", "),
        fields.join(" ")
    );
    let names: Vec<(String, String)> = repos
        .iter()
        .enumerate()
        .flat_map(|(i, (owner, name))| [(format!("o{i}"), owner.clone()), (format!("n{i}"), name.clone())])
        .collect();
    let strings: Vec<(&str, &str)> = names.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
    gh.graphql(&query, &strings, &[]).await
}

/// A repo GitHub would not answer for carries its error; the rest still answer.
fn read_repo(answer: &Value, index: usize, owner: &str, name: &str) -> Value {
    let key = format!("r{index}");
    let repo = &answer["data"][&key];
    if repo.is_null() {
        let errors = answer["errors"].as_array().cloned().unwrap_or_default();
        let why = errors.iter().find(|error| error["path"][0] == key.as_str()).and_then(|error| error["message"].as_str());
        return json!({ "repo": format!("{owner}/{name}"), "url": format!("https://github.com/{owner}/{name}"), "prs": [], "error": why.unwrap_or("GitHub did not answer for this repo") });
    }
    let prs: Vec<Value> = repo["pullRequests"]["nodes"].as_array().into_iter().flatten().map(pull).collect();
    json!({
        "repo": format!("{owner}/{name}"),
        "url": repo["url"],
        "default_branch": repo["defaultBranchRef"]["name"],
        "merge_method": default_method(repo),
        "prs": prs,
    })
}

fn pull(pr: &Value) -> Value {
    let labels: Vec<Value> = pr["labels"]["nodes"].as_array().into_iter().flatten().map(|label| json!({ "name": label["name"], "color": label["color"] })).collect();
    json!({
        "number": pr["number"],
        "title": pr["title"],
        "url": pr["url"],
        "body": pr["body"],
        // A deleted account comes back as no author at all; GitHub itself calls it "ghost".
        "author": { "login": pr["author"]["login"].as_str().unwrap_or("ghost"), "avatar_url": pr["author"]["avatarUrl"] },
        "head": pr["headRefName"],
        "base": pr["baseRefName"],
        "head_sha": pr["headRefOid"],
        "draft": pr["isDraft"].as_bool().unwrap_or(false),
        "labels": labels,
        "additions": pr["additions"],
        "deletions": pr["deletions"],
        "changed_files": pr["changedFiles"],
        "created_at": pr["createdAt"],
        "updated_at": pr["updatedAt"],
    })
}

/// Each pull request's files, at the head commit the list just read, a few at a time.
/// A diff GitHub will not produce leaves `files` off that one and says why in
/// `diff_error`; the list itself still answers.
async fn attach_diffs(gh: &Gh, repos: &mut [Value]) {
    let gate = Arc::new(tokio::sync::Semaphore::new(DIFFS_AT_ONCE));
    let mut fetches = tokio::task::JoinSet::new();
    for (r, repo) in repos.iter().enumerate() {
        let Some((owner, name)) = repo["repo"].as_str().and_then(repo_of) else { continue };
        for (p, pr) in repo["prs"].as_array().into_iter().flatten().enumerate() {
            let (base, sha) = (pr["base"].as_str().unwrap_or_default().to_string(), pr["head_sha"].as_str().unwrap_or_default().to_string());
            let (gh, gate, owner, name) = (gh.clone(), gate.clone(), owner.clone(), name.clone());
            fetches.spawn(async move {
                let _turn = gate.acquire_owned().await;
                (r, p, diff::at(&gh, &owner, &name, &base, &sha).await)
            });
        }
    }
    while let Some(Ok((r, p, files))) = fetches.join_next().await {
        let pr = &mut repos[r]["prs"][p];
        match files {
            Ok(files) => pr["files"] = json!(files),
            Err(failure) => pr["diff_error"] = json!(failure.error),
        }
    }
}
