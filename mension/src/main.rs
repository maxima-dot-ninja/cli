//! Eliot's side of artefact comments: the threads waiting on him, every thread in a project, and his
//! reply — through the api's machine-token routes. It decides nothing about when to run; the
//! daemon's scheduled job does. Success is one pretty JSON object on stdout. Failure is one
//! `{"ok":false,"error":…}` line on stderr, with an exit code that says which kind.

mod api;
mod select;

use serde_json::{json, Value};

const USAGE: &str = "\
mension — Eliot's side of artefact comments

  mension unanswered SLUG                            the threads waiting on Eliot in a project
  mension list       SLUG                            every thread on every artefact in a project
  mension reply      SLUG --comment ID --body TEXT   reply as Eliot in that comment's thread
  mension doctor                                     is the token set, and which api it calls

SLUG is the project's slug or id. A thread is a first comment and its replies, oldest first, and
the thread with the newest comment comes first. Each comment is exactly as the api sent it.

Success prints one JSON object on stdout. Failure prints {\"ok\":false,\"error\":\"…\"} on stderr:
  1  the api refused, answered something unreadable, or could not be reached
  2  nothing was sent: the token is missing or the command line is wrong
  3  the api did not answer within 8 seconds

Configuration:
  MENSION_MACHINE_TOKEN  the api's machine token, in ~/.vaulty/.secrets/mension.env
  MENSION_API_URL        the api; else NINJA_API_URL; else https://api.maxima.ninja/v1
";

/// Exit codes, so a bash caller can tell a refusal from a setup problem from a slow api.
pub const FAILED: i32 = 1;
pub const SETUP: i32 = 2;
pub const TIMEOUT: i32 = 3;

pub struct Fail {
    code: i32,
    error: String,
}

impl Fail {
    pub fn new(code: i32, error: impl Into<String>) -> Self {
        Fail { code, error: error.into() }
    }

    fn exit(self) -> ! {
        eprintln!("{}", json!({ "ok": false, "error": self.error }));
        std::process::exit(self.code)
    }
}

/// Parsed by hand: flags sit anywhere, everything else is a positional.
#[derive(Default)]
struct Cli {
    args: Vec<String>,
    comment: String,
    body: String,
    help: bool,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Cli {
    let mut cli = Cli::default();
    while let Some(token) = argv.next() {
        let mut next = || argv.next().unwrap_or_default();
        match token.as_str() {
            "--help" | "-h" => cli.help = true,
            "--comment" | "-c" => cli.comment = next(),
            "--body" | "-b" => cli.body = next(),
            _ => cli.args.push(token),
        }
    }
    cli
}

impl Cli {
    fn arg(&self, i: usize) -> &str {
        self.args.get(i).map(String::as_str).unwrap_or("")
    }

    fn slug(&self) -> Result<&str, Fail> {
        let (cmd, slug) = (self.arg(0), self.arg(1));
        if slug.is_empty() {
            return Err(Fail::new(SETUP, format!("{cmd} needs a project: mension {cmd} SLUG")));
        }
        Ok(slug)
    }
}

/// The api's flat list, newest first, as threads. A reply's thread is its `parent_id`; a first
/// comment is its own. Threads keep the api's order, so the newest activity leads, and each one
/// reads oldest first, the way the conversation happened.
fn threads(comments: Value) -> Result<Value, Fail> {
    let Value::Array(comments) = comments else {
        return Err(Fail::new(FAILED, "the api answered with something that is not a list of comments"));
    };
    let mut threads: Vec<(String, Vec<Value>)> = Vec::new();
    for comment in comments {
        let root = comment["parent_id"].as_str().or(comment["id"].as_str()).unwrap_or_default().to_string();
        match threads.iter_mut().find(|(id, _)| *id == root) {
            Some((_, said)) => said.push(comment),
            None => threads.push((root, vec![comment])),
        }
    }
    let threads: Vec<Value> = threads
        .into_iter()
        .map(|(root, mut said)| {
            said.reverse();
            json!({ "root_id": root, "artefact_id": said[0]["artefact_id"], "artefact": said[0]["artefact"], "comments": said })
        })
        .collect();
    Ok(json!({ "ok": true, "threads": threads }))
}

fn unanswered(cli: &Cli) -> Result<Value, Fail> {
    let mut url = api::comments(cli.slug()?, "list")?;
    url.query_pairs_mut().append_pair("unanswered_eliot", "true");
    threads(api::get(url)?)
}

fn list(cli: &Cli) -> Result<Value, Fail> {
    threads(api::get(api::comments(cli.slug()?, "list")?)?)
}

/// The server puts the reply under the thread's first comment, so any comment in it will do.
fn reply(cli: &Cli) -> Result<Value, Fail> {
    let slug = cli.slug()?;
    if cli.comment.is_empty() || cli.body.trim().is_empty() {
        return Err(Fail::new(SETUP, "reply needs both: mension reply SLUG --comment ID --body TEXT"));
    }
    let said = json!({ "comment_id": cli.comment, "body": cli.body });
    let comment = api::post(api::comments(slug, "reply")?, &said)?;
    Ok(json!({ "ok": true, "comment": comment }))
}

/// No request: only whether the token is there, and which api the other commands would call.
fn doctor(_: &Cli) -> Result<Value, Fail> {
    let token = api::token()?;
    Ok(json!({ "ok": true, "api_url": api::base(), "token": "present", "masked": masked(&token) }))
}

/// The first and last four characters, so two tokens can be told apart and neither is shown.
fn masked(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() < 16 {
        return "…".to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

type Action = fn(&Cli) -> Result<Value, Fail>;

const COMMANDS: [(&str, Action); 4] =
    [("unanswered", unanswered), ("list", list), ("reply", reply), ("doctor", doctor)];

/// The menu asks for what its command needs, builds the command line a script would have typed,
/// and runs that. An empty flag means the answer is a positional.
type Asks = &'static [(&'static str, &'static str)];

const MENU: [(&str, &str, Asks); 4] = [
    ("Threads waiting on Eliot", "unanswered", &[("", "Project slug:")]),
    ("Every thread in a project", "list", &[("", "Project slug:")]),
    ("Reply as Eliot", "reply", &[("", "Project slug:"), ("--comment", "Comment id:"), ("--body", "Reply:")]),
    ("Check the token and the api", "doctor", &[]),
];

fn menu() -> Result<Value, Fail> {
    let asked = |e: anyhow::Error| Fail::new(FAILED, format!("{e:#}"));
    let names: Vec<String> = MENU.iter().map(|(name, ..)| name.to_string()).collect();
    let (_, cmd, asks) = MENU[select::select("mension", &names).map_err(asked)?];
    let mut argv = vec![cmd.to_string()];
    for (flag, question) in asks {
        let answer = select::prompt(question).map_err(asked)?;
        argv.extend(Some(flag.to_string()).filter(|f| !f.is_empty()));
        argv.push(answer);
    }
    run(&parse(argv.into_iter()))
}

fn run(cli: &Cli) -> Result<Value, Fail> {
    let cmd = cli.arg(0);
    match COMMANDS.iter().find(|(name, _)| *name == cmd) {
        Some((_, action)) => action(cli),
        None => Err(Fail::new(SETUP, format!("unknown command: {cmd} — run mension --help"))),
    }
}

fn main() {
    let cli = parse(std::env::args().skip(1));
    if cli.help || (cli.arg(0).is_empty() && !select::is_interactive()) {
        print!("{USAGE}");
        return;
    }
    let outcome = if cli.arg(0).is_empty() { menu() } else { run(&cli) };
    match outcome {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default()),
        Err(fail) => fail.exit(),
    }
}
