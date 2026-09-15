//! Local project files for an agent: list them, read one, grep them, or ingest
//! a whole folder — always inside one root, never a dotfile or a secret unless
//! asked, and JSON on `--json` so nothing has to be parsed twice.

mod grep;
mod read;
mod select;
mod walk;

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;
use walk::Filter;

const USAGE: &str = "\
ingest — read local project files safely

  ingest list   [DIR]      [--depth N] [--include GLOBS] [--exclude GLOBS]
  ingest read   FILE       [--root DIR] [--offset N] [--max-bytes N] [--binary]
  ingest grep   PATTERN [DIR] [--literal] [-i] [--max N] [--include GLOBS] [--exclude GLOBS]
  ingest ingest [DIR]      [--depth N] [--include GLOBS] [--exclude GLOBS] [--max-bytes N]

Flags that work everywhere:
  --json            print JSON instead of text
  --hidden          include dotfiles and dot-folders
  --allow-secrets   include env files, keys, certificates and credential folders
  --root DIR        the folder nothing may escape (default: INGEST_ROOT, else the current folder)

GLOBS is comma-separated. `*` crosses `/`, so `*.rs` matches src/main.rs.

Environment:
  INGEST_ROOT        default root
  INGEST_EXCLUDE     default excludes, comma-separated (set to \"\" to exclude nothing)
                     default: .git,node_modules,target,dist,build,out,.next,.turbo,.cache,__pycache__,.venv,venv,.DS_Store
  INGEST_MAX_BYTES   bytes served per file when --max-bytes is not given (default 262144)
";

/// Parsed by hand: flags sit anywhere, everything else is a positional.
#[derive(Default)]
struct Cli {
    args: Vec<String>,
    root: String,
    depth: String,
    offset: String,
    max_bytes: String,
    max: String,
    include: Vec<String>,
    exclude: Vec<String>,
    json: bool,
    binary: bool,
    literal: bool,
    ignore_case: bool,
    hidden: bool,
    secrets: bool,
    help: bool,
}

fn parse(mut argv: impl Iterator<Item = String>) -> Cli {
    let mut cli = Cli::default();
    while let Some(token) = argv.next() {
        let mut next = || argv.next().unwrap_or_default();
        match token.as_str() {
            "--json" => cli.json = true,
            "--binary" | "--base64" => cli.binary = true,
            "--literal" | "-F" => cli.literal = true,
            "--ignore-case" | "-i" => cli.ignore_case = true,
            "--hidden" => cli.hidden = true,
            "--allow-secrets" => cli.secrets = true,
            "--help" | "-h" => cli.help = true,
            "--root" => cli.root = next(),
            "--depth" | "-d" => cli.depth = next(),
            "--offset" => cli.offset = next(),
            "--max-bytes" => cli.max_bytes = next(),
            "--max" | "-n" => cli.max = next(),
            "--include" => cli.include.push(next()),
            "--exclude" => cli.exclude.push(next()),
            _ => cli.args.push(token),
        }
    }
    cli
}

impl Cli {
    fn arg(&self, i: usize) -> &str {
        self.args.get(i).map(String::as_str).unwrap_or("")
    }

    fn number<T: std::str::FromStr>(&self, text: &str, flag: &str) -> Result<Option<T>> {
        if text.is_empty() {
            return Ok(None);
        }
        match text.parse() {
            Ok(n) => Ok(Some(n)),
            Err(_) => bail!("{flag} wants a whole number, got {text:?}"),
        }
    }

    /// The root for a walking command: --root, else the positional folder, else the defaults.
    fn walk_root(&self, positional: usize) -> Result<std::path::PathBuf> {
        let dir = if self.root.is_empty() { self.arg(positional) } else { &self.root };
        walk::root(dir)
    }

    fn filter(&self) -> Result<Filter> {
        Filter::new(&self.include, &self.exclude, self.hidden, self.secrets)
    }

    fn per_file_limit(&self) -> Result<usize> {
        Ok(self.number::<usize>(&self.max_bytes, "--max-bytes")?.filter(|&n| n > 0).unwrap_or(read::max_bytes()))
    }
}

fn emit(cli: &Cli, value: &Value, text: impl FnOnce() -> String) -> Result<()> {
    if cli.json {
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(());
    }
    println!("{}", text());
    Ok(())
}

fn list(cli: &Cli) -> Result<()> {
    let root = cli.walk_root(1)?;
    let depth = cli.number(&cli.depth, "--depth")?;
    let files = walk::files(&root, depth, &cli.filter()?);
    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    emit(cli, &json!(paths), || paths.join("\n"))
}

fn read_file(cli: &Cli) -> Result<()> {
    let root = walk::root(&cli.root)?;
    let (abs, rel) = walk::inside(&root, cli.arg(1))?;
    if let Some(why) = Filter::safety_only(cli.hidden, cli.secrets).refusal(&rel) {
        bail!("refused: {why}");
    }
    let offset = cli.number(&cli.offset, "--offset")?.unwrap_or(0);
    let slice = read::slice(&abs, offset, cli.per_file_limit()?)?;
    let value = read::json(&abs, &slice, cli.binary);
    emit(cli, &value, || value["content"].as_str().unwrap_or("").to_string())
}

fn grep_files(cli: &Cli) -> Result<()> {
    let pattern = cli.arg(1);
    if pattern.is_empty() {
        bail!("grep needs a pattern: ingest grep PATTERN [DIR]");
    }
    let re = grep::pattern(pattern, cli.literal, cli.ignore_case)?;
    let root = cli.walk_root(2)?;
    let max = cli.number(&cli.max, "--max")?.unwrap_or(500);
    let files = walk::files(&root, None, &cli.filter()?);
    let hits = grep::search(&files, &re, max);
    let lines = || {
        hits.iter()
            .map(|h| format!("{}:{}: {}", h["path"].as_str().unwrap_or(""), h["line"], h["snippet"].as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    emit(cli, &json!(hits), lines)
}

/// One file of the ingest reply. Truncated files are marked, not dropped.
fn ingest_entry(path: &Path, limit: usize) -> Result<Value> {
    let slice = read::slice(path, 0, limit)?;
    let binary = read::is_binary(&slice.bytes);
    Ok(json!({
        "path": path.display().to_string(),
        "content": read::encode(&slice.bytes, binary),
        "binary": binary,
        "truncated": slice.truncated(),
    }))
}

fn ingest_dir(cli: &Cli) -> Result<()> {
    let root = cli.walk_root(1)?;
    let depth = cli.number(&cli.depth, "--depth")?;
    let limit = cli.per_file_limit()?;
    let files = walk::files(&root, depth, &cli.filter()?);
    let entries: Vec<Value> = files.iter().map(|p| ingest_entry(p, limit)).collect::<Result<_>>()?;
    let text = || {
        entries
            .iter()
            .map(|e| {
                let tag = if e["binary"].as_bool() == Some(true) { " (base64)" } else { "" };
                format!("=== {}{tag} ===\n{}", e["path"].as_str().unwrap_or(""), e["content"].as_str().unwrap_or(""))
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    emit(cli, &json!(entries), text)
}

type Action = fn(&Cli) -> Result<()>;

const COMMANDS: [(&str, Action); 4] =
    [("list", list), ("read", read_file), ("grep", grep_files), ("ingest", ingest_dir)];

/// The wizard asks for the one thing each command cannot guess, then runs it
/// with the same code path as the command line.
const MENU: [(&str, &str); 4] = [
    ("List files in a folder", "Folder (blank for here):"),
    ("Read one file", "File path:"),
    ("Grep file contents", "Pattern:"),
    ("Ingest a whole folder", "Folder (blank for here):"),
];

fn menu(mut cli: Cli) -> Result<()> {
    let names: Vec<String> = MENU.iter().map(|(name, _)| name.to_string()).collect();
    let pick = select::select("ingest", &names)?;
    let answer = select::prompt(MENU[pick].1)?;
    cli.args = vec![COMMANDS[pick].0.to_string(), answer];
    COMMANDS[pick].1(&cli)
}

fn main() -> Result<()> {
    let cli = parse(std::env::args().skip(1));
    if cli.help {
        print!("{USAGE}");
        return Ok(());
    }
    let cmd = cli.arg(0);
    if cmd.is_empty() && select::is_interactive() {
        return menu(cli);
    }
    match COMMANDS.iter().find(|(name, _)| *name == cmd) {
        Some((_, run)) => run(&cli),
        None if cmd.is_empty() => {
            print!("{USAGE}");
            Ok(())
        }
        None => {
            eprint!("{USAGE}");
            bail!("unknown command: {cmd}")
        }
    }
}
