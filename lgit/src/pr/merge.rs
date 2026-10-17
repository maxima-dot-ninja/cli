//! `lgit pr merge`: merges one pull request as the `gh` login, once, or says why not.
//!
//! It reads the pull request first and refuses anything GitHub would refuse — a draft,
//! a conflict, failing or running checks, a review the branch rules require — and
//! anything that moved since it was read: the merge carries the head commit as
//! `expectedHeadOid`, so a push in between makes GitHub refuse it rather than merge
//! code nobody looked at. It never retries. A pull request that is already merged
//! answers merged, with its merge commit, so running it twice is safe.

use super::gh::{default_method, Failure, Gh, METHOD_FIELDS};
use serde_json::{json, Value};

/// The fields a pull request's mergeability is judged on. `lgit pr diff` reads them for
/// the Merge button, and `lgit pr merge` reads them again just before it merges.
pub const STATE: &str = "id url state merged isDraft mergeable mergeStateStatus reviewDecision headRefOid baseRefName \
    mergeCommit { oid } mergedBy { login } commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }";

/// The commit message GitHub's own button fills in for this login, which follows the
/// repo's setting for squash messages. A rebase writes no commit of its own.
const MESSAGES: &str = "squashHeadline: viewerMergeHeadlineText(mergeType: SQUASH) squashBody: viewerMergeBodyText(mergeType: SQUASH) \
    mergeHeadline: viewerMergeHeadlineText(mergeType: MERGE) mergeBody: viewerMergeBodyText(mergeType: MERGE)";

const MERGE: &str = "mutation($id: ID!, $method: PullRequestMergeMethod!, $sha: GitObjectID!, $headline: String, $body: String) { \
    mergePullRequest(input: { pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $sha, commitHeadline: $headline, commitBody: $body }) { \
    pullRequest { merged url mergeCommit { oid } } } }";

/// What was asked for: the pull request, the head commit it was reviewed at, and a
/// method other than the repo's default.
pub struct Ask<'a> {
    pub owner: &'a str,
    pub name: &'a str,
    pub number: u64,
    pub sha: Option<&'a str>,
    pub method: Option<&'a str>,
}

/// One refusal: the test, its reason code and what it says. Checked in order; the first
/// that holds is the answer.
type Rule = (fn(&Value) -> bool, &'static str, &'static str);

const RULES: &[Rule] = &[
    (|pr| pr["merged"] == true, "already_merged", "It is already merged."),
    (|pr| pr["state"] != "OPEN", "closed", "It was closed without being merged."),
    (|pr| pr["isDraft"] == true, "draft", "It is a draft. Mark it ready for review first."),
    (|pr| pr["mergeable"] == "CONFLICTING" || pr["mergeStateStatus"] == "DIRTY", "conflicts", "It conflicts with its base branch. Resolve the conflicts first."),
    (|pr| pr["mergeable"] == "UNKNOWN" || pr["mergeStateStatus"] == "UNKNOWN", "unknown", "GitHub is still working out whether it can merge. Try again in a few seconds."),
    (|pr| matches!(checks(pr), "FAILURE" | "ERROR"), "checks_failing", "Its checks are failing."),
    (|pr| matches!(checks(pr), "PENDING" | "EXPECTED"), "checks_pending", "Its checks are still running."),
    (|pr| blocked(pr) && pr["reviewDecision"] == "CHANGES_REQUESTED", "changes_requested", "A reviewer asked for changes."),
    (|pr| blocked(pr) && pr["reviewDecision"] == "REVIEW_REQUIRED", "review_required", "Its branch rules need an approving review first."),
    (|pr| blocked(pr), "blocked", "Its branch rules block the merge."),
    (|pr| pr["mergeStateStatus"] == "BEHIND", "behind", "Its branch is behind the base, and the repo wants it up to date first."),
];

fn checks(pr: &Value) -> &str {
    pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["state"].as_str().unwrap_or_default()
}

fn blocked(pr: &Value) -> bool {
    pr["mergeStateStatus"] == "BLOCKED"
}

/// Why GitHub would refuse to merge it right now, or nothing when it would merge.
fn verdict(pr: &Value) -> Option<Failure> {
    RULES.iter().find(|(test, _, _)| test(pr)).map(|(_, reason, why)| Failure::new(reason, *why))
}

/// What the Merge button shows before anyone presses it: whether it would merge and with
/// which method, or why not, in the same words `lgit pr merge` would refuse with.
pub fn readiness(repo: &Value, pr: &Value) -> Value {
    let method = default_method(repo);
    let refusal = verdict(pr).or_else(|| method.is_none().then(no_method));
    match refusal {
        None => json!({ "ok": true, "method": method }),
        Some(failure) => json!({ "ok": false, "method": method, "reason": failure.reason, "error": failure.error }),
    }
}

fn no_method() -> Failure {
    Failure::new("method_not_allowed", "The repo allows no merge method this login can use.")
}

/// Merges it, or answers a `Failure` naming why not.
pub async fn run(gh: &Gh, ask: Ask<'_>) -> Result<Value, Failure> {
    let query = format!(
        "query($o: String!, $n: String!, $num: Int!) {{ viewer {{ login }} repository(owner: $o, name: $n) {{ {METHOD_FIELDS} pullRequest(number: $num) {{ {STATE} {MESSAGES} }} }} }}"
    );
    let read = gh.graphql(&query, &[("o", ask.owner), ("n", ask.name)], &[("num", ask.number)]).await?;
    let repo = &read["data"]["repository"];
    let pr = &repo["pullRequest"];
    if pr.is_null() {
        return Err(Failure::new("not_found", format!("{}/{} has no pull request #{}, or this login cannot see it", ask.owner, ask.name, ask.number)));
    }
    if pr["merged"] == true {
        return Ok(merged(&ask, pr["mergeCommit"]["oid"].clone(), pr["url"].clone(), pr["mergedBy"]["login"].clone(), Value::Null, true));
    }
    let head = pr["headRefOid"].as_str().unwrap_or_default();
    if ask.sha.is_some_and(|sha| sha != head) {
        return Err(Failure::new("head_changed", "Someone pushed to it since you looked. Read the new diff first."));
    }
    if let Some(failure) = verdict(pr) {
        return Err(failure);
    }
    let method = pick(repo, ask.method)?;
    let viewer = read["data"]["viewer"]["login"].as_str().unwrap_or_default();
    send(gh, &ask, pr, method, head, viewer).await
}

/// The method asked for when the repo allows it, else the repo's default.
fn pick(repo: &Value, asked: Option<&str>) -> Result<&'static str, Failure> {
    let Some(asked) = asked else { return default_method(repo).ok_or_else(no_method) };
    let allowed = [("squash", "squashMergeAllowed"), ("merge", "mergeCommitAllowed"), ("rebase", "rebaseMergeAllowed")];
    let found = allowed.iter().find(|(method, _)| *method == asked).filter(|(_, field)| repo[*field] == true);
    found.map(|(method, _)| *method).ok_or_else(|| Failure::new("method_not_allowed", format!("The repo does not allow {asked} merges.")))
}

async fn send(gh: &Gh, ask: &Ask<'_>, pr: &Value, method: &'static str, head: &str, viewer: &str) -> Result<Value, Failure> {
    let id = pr["id"].as_str().unwrap_or_default();
    let upper = method.to_uppercase();
    let mut strings = vec![("id", id), ("method", upper.as_str()), ("sha", head)];
    let (headline, body) = (pr[format!("{method}Headline")].as_str(), pr[format!("{method}Body")].as_str());
    strings.extend(headline.map(|text| ("headline", text)));
    strings.extend(body.map(|text| ("body", text)));
    let answer = gh.graphql(MERGE, &strings, &[]).await.map_err(refused)?;
    let done = &answer["data"]["mergePullRequest"]["pullRequest"];
    if done["merged"] != true {
        let why = answer["errors"][0]["message"].as_str().unwrap_or("GitHub did not merge it");
        return Err(refused(Failure::new("refused", why)));
    }
    Ok(merged(ask, done["mergeCommit"]["oid"].clone(), done["url"].clone(), json!(viewer), json!(method), false))
}

/// GitHub's words for a refused merge, sorted into the reasons a caller can act on. A
/// timeout is the one case where nobody knows: the merge may have landed.
fn refused(failure: Failure) -> Failure {
    if failure.reason == "timed_out" {
        return Failure::new("timed_out", "GitHub did not answer in time, so it may or may not have merged. Read the list again to see.");
    }
    let known: &[(&str, &'static str, &str)] = &[
        ("head branch was modified", "head_changed", "Someone pushed to it since you looked. Read the new diff first."),
        ("not mergeable", "not_mergeable", "GitHub says it is not mergeable."),
        ("merge queue", "merge_queue", "This repo merges through a merge queue, which lgit does not drive."),
        ("permission", "no_permission", "This login cannot merge into this repo."),
    ];
    let said = failure.error.to_lowercase();
    match known.iter().find(|(words, _, _)| said.contains(words)) {
        Some((_, reason, why)) => Failure::new(reason, format!("{why} GitHub said: {}", failure.error)),
        None => failure,
    }
}

fn merged(ask: &Ask<'_>, sha: Value, url: Value, by: Value, method: Value, already: bool) -> Value {
    json!({
        "merged": true,
        "already": already,
        "repo": format!("{}/{}", ask.owner, ask.name),
        "number": ask.number,
        "method": method,
        "sha": sha,
        "url": url,
        "merged_by": by,
    })
}
