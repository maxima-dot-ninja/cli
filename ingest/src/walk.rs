//! Where ingest is allowed to look: the root, the safety filter, and the walk.

use anyhow::{bail, Context, Result};
use glob::{MatchOptions, Pattern};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Skipped unless `INGEST_EXCLUDE` says otherwise. Set it to "" to skip nothing.
pub const DEFAULT_EXCLUDES: &str =
    ".git,node_modules,target,dist,build,out,.next,.turbo,.cache,__pycache__,.venv,venv,.DS_Store";

/// A file counts as a secret when any of these describe its name or one of
/// its parent folders. Reading one needs --allow-secrets.
const SECRET_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "jks", "keystore"];
const SECRET_PREFIXES: &[&str] = &[".env", "id_rsa", "id_ed25519", "id_ecdsa", "id_dsa"];
const SECRET_NAMES: &[&str] = &[".netrc", ".npmrc", ".pypirc", ".htpasswd", "credentials", "credentials.json"];
const SECRET_SUBSTRINGS: &[&str] = &["credential", "secret"];
const SECRET_DIRS: &[&str] = &[".ssh", ".aws", ".gnupg", ".kube", ".docker"];

/// Globs match the whole relative path with `*` allowed to cross `/`, so
/// `*.rs` finds `src/main.rs` and `src/**` finds everything under src.
const GLOB: MatchOptions =
    MatchOptions { case_sensitive: true, require_literal_separator: false, require_literal_leading_dot: false };

/// The folder every path is checked against, canonicalized so `..` and
/// symlinks cannot reach past it.
pub fn root(arg: &str) -> Result<PathBuf> {
    let raw = match arg {
        "" => std::env::var("INGEST_ROOT").ok().filter(|dir| !dir.is_empty()).unwrap_or(".".to_string()),
        dir => dir.to_string(),
    };
    let root = Path::new(&raw).canonicalize().with_context(|| format!("root not found: {raw}"))?;
    if !root.is_dir() {
        bail!("root is not a directory: {}", root.display());
    }
    Ok(root)
}

/// A file path, absolute or relative to the root, resolved and proven to sit
/// inside the root. Returns the absolute path and the relative one.
pub fn inside(root: &Path, arg: &str) -> Result<(PathBuf, PathBuf)> {
    if arg.is_empty() {
        bail!("no path given");
    }
    let joined = if Path::new(arg).is_absolute() { PathBuf::from(arg) } else { root.join(arg) };
    let abs = joined.canonicalize().with_context(|| format!("not found: {arg}"))?;
    let rel = match abs.strip_prefix(root) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => bail!("refused: {arg} resolves outside the root {}", root.display()),
    };
    Ok((abs, rel))
}

fn name_of<'a>(component: &'a std::path::Component<'a>) -> &'a str {
    component.as_os_str().to_str().unwrap_or("")
}

fn extension_of(name: &str) -> &str {
    name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

/// Why one path component looks like a secret, or None.
fn secret_reason(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if SECRET_DIRS.contains(&lower.as_str()) {
        return Some("a credentials folder");
    }
    if SECRET_NAMES.contains(&lower.as_str()) {
        return Some("a credentials file");
    }
    if SECRET_PREFIXES.iter().any(|prefix| lower.starts_with(prefix)) {
        return Some("an env file or private key");
    }
    if SECRET_EXTENSIONS.contains(&extension_of(&lower)) {
        return Some("a key or certificate");
    }
    if SECRET_SUBSTRINGS.iter().any(|word| lower.contains(word)) {
        return Some("named like a secret");
    }
    None
}

pub struct Filter {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
    hidden: bool,
    secrets: bool,
}

/// Comma-separated globs, blanks dropped.
pub fn split_globs(specs: &[String]) -> Vec<String> {
    specs.iter().flat_map(|spec| spec.split(',')).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

fn patterns(globs: &[String]) -> Result<Vec<Pattern>> {
    globs.iter().map(|g| Pattern::new(g).with_context(|| format!("bad glob: {g}"))).collect()
}

impl Filter {
    /// `include` and `exclude` come from the command line; the default excludes
    /// from INGEST_EXCLUDE (or the built-in list) always sit underneath them.
    pub fn new(include: &[String], exclude: &[String], hidden: bool, secrets: bool) -> Result<Self> {
        let defaults = std::env::var("INGEST_EXCLUDE").unwrap_or(DEFAULT_EXCLUDES.to_string());
        let mut excludes = split_globs(&[defaults]);
        excludes.extend(split_globs(exclude));
        Ok(Filter {
            include: patterns(&split_globs(include))?,
            exclude: patterns(&excludes)?,
            hidden,
            secrets,
        })
    }

    /// Only the hidden and secret rules — for reading a file named outright,
    /// where an exclude glob meant for the walk should not get in the way.
    pub fn safety_only(hidden: bool, secrets: bool) -> Self {
        Filter { include: vec![], exclude: vec![], hidden, secrets }
    }

    /// Why this relative path is off limits, or None when it may be served.
    /// Applied to folders during the walk too, so a refused folder is pruned.
    pub fn refusal(&self, rel: &Path) -> Option<String> {
        let rel_str = rel.to_string_lossy();
        for component in rel.components() {
            let name = name_of(&component);
            if !self.hidden && name.starts_with('.') {
                return Some(format!("{rel_str} is hidden (pass --hidden to include dotfiles)"));
            }
            if !self.secrets {
                if let Some(why) = secret_reason(name) {
                    return Some(format!("{rel_str} looks like {why} (pass --allow-secrets to read it anyway)"));
                }
            }
            if self.exclude.iter().any(|p| p.matches_with(name, GLOB)) {
                return Some(format!("{rel_str} is excluded"));
            }
        }
        if self.exclude.iter().any(|p| p.matches_with(&rel_str, GLOB)) {
            return Some(format!("{rel_str} is excluded"));
        }
        None
    }

    /// Include globs only narrow the files; no globs means every file.
    fn included(&self, rel: &Path) -> bool {
        let rel_str = rel.to_string_lossy();
        self.include.is_empty() || self.include.iter().any(|p| p.matches_with(&rel_str, GLOB))
    }
}

/// Every regular file under the root the filter allows, sorted, absolute.
/// Symlinks are neither followed nor listed, so nothing can point outside.
pub fn files(root: &Path, depth: Option<usize>, filter: &Filter) -> Vec<PathBuf> {
    let walk = WalkDir::new(root).min_depth(1).max_depth(depth.unwrap_or(usize::MAX)).follow_links(false).sort_by_file_name();
    walk.into_iter()
        .filter_entry(|entry| {
            let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
            filter.refusal(rel).is_none()
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| filter.included(entry.path().strip_prefix(root).unwrap_or(entry.path())))
        .map(|entry| entry.into_path())
        .collect()
}
