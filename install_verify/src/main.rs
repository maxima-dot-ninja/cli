use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use clap::Parser;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LAUNCHD_LABEL: &str = "com.vaulty.daemon";
const PING_TIMEOUT: Duration = Duration::from_secs(15);

/// Verify the vaulty daemon after `./bin/install`: PID, binary, start time, and a live ping.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// When `./bin/install` was invoked, as ISO-8601 (e.g. 2026-09-15T11:17:00+02:00). Defaults to now.
    #[arg(long)]
    install_time: Option<String>,

    /// Command used as the responsive ping.
    #[arg(long, default_value = "vaulty status")]
    ping_command: String,
}

#[derive(Serialize, Default)]
struct Report {
    pid: Option<u32>,
    binary_path: Option<String>,
    binary_resolved: Option<String>,
    binary_ok: bool,
    start_time: Option<String>,
    install_time: Option<String>,
    start_after_install: bool,
    ping_command: String,
    ping_ok: bool,
    ping_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() {
    let args = Args::parse();
    let mut report = Report {
        ping_command: args.ping_command.clone(),
        ..Default::default()
    };
    let outcome = run(&args, &mut report);
    if let Err(e) = outcome {
        report.error = Some(format!("{e:#}"));
    }
    println!("{}", serde_json::to_string_pretty(&report).expect("report serializes"));
    std::process::exit(if report.error.is_none() { 0 } else { 1 });
}

fn run(args: &Args, r: &mut Report) -> Result<()> {
    let install_time = parse_install_time(args.install_time.as_deref())?;
    r.install_time = Some(install_time.to_rfc3339());

    let pid = find_pid()?;
    r.pid = Some(pid);

    let binary_path = ps_field(pid, "comm=")?;
    r.binary_path = Some(binary_path.clone());
    let resolved = std::fs::canonicalize(&binary_path)
        .with_context(|| format!("cannot resolve binary path {binary_path}"))?;
    r.binary_resolved = Some(resolved.display().to_string());
    r.binary_ok = resolved == expected_binary()?;

    let start_time = parse_lstart(&ps_field(pid, "lstart=")?)?;
    r.start_time = Some(start_time.to_rfc3339());
    r.start_after_install = start_time >= install_time;

    let (ping_ok, ping_output) = ping(&args.ping_command)?;
    r.ping_ok = ping_ok;
    r.ping_output = Some(ping_output);

    let checks = [
        (r.binary_ok, format!("pid {pid} runs {} instead of {}", resolved.display(), expected_binary()?.display())),
        (r.start_after_install, format!("daemon started {} before install time {}", start_time.to_rfc3339(), install_time.to_rfc3339())),
        (r.ping_ok, format!("ping `{}` failed", args.ping_command)),
    ];
    let failures: Vec<String> = checks.into_iter().filter(|(ok, _)| !ok).map(|(_, msg)| msg).collect();
    if failures.is_empty() {
        return Ok(());
    }
    bail!("{}", failures.join("; "))
}

fn parse_install_time(raw: Option<&str>) -> Result<DateTime<Local>> {
    let Some(raw) = raw else {
        return Ok(Local::now());
    };
    let raw = raw.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Ok(t.with_timezone(&Local));
    }
    let naive_formats = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M"];
    let naive = naive_formats
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(raw, f).ok())
        .ok_or_else(|| anyhow!("--install-time {raw:?} is not ISO-8601"))?;
    Local
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| anyhow!("--install-time {raw:?} is ambiguous in the local timezone"))
}

fn find_pid() -> Result<u32> {
    let out = Command::new("launchctl").args(["list", LAUNCHD_LABEL]).output().context("running launchctl")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let from_launchd = text
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("\"PID\" = "))
        .and_then(|v| v.trim_end_matches(';').trim().parse::<u32>().ok());
    if let Some(pid) = from_launchd {
        return Ok(pid);
    }
    let out = Command::new("pgrep").args(["-f", "vaulty serve"]).output().context("running pgrep")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let pids: Vec<u32> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
    match pids.as_slice() {
        [pid] => Ok(*pid),
        [] => bail!("no vaulty daemon: launchctl has no PID for {LAUNCHD_LABEL} and `vaulty serve` is not running"),
        many => bail!("multiple `vaulty serve` processes and no launchd PID: {many:?}"),
    }
}

fn ps_field(pid: u32, field: &str) -> Result<String> {
    let out = Command::new("ps").args(["-p", &pid.to_string(), "-o", field]).output().context("running ps")?;
    let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if value.is_empty() {
        bail!("ps returned nothing for pid {pid} field {field}; the process is gone");
    }
    Ok(value)
}

fn parse_lstart(raw: &str) -> Result<DateTime<Local>> {
    let naive = NaiveDateTime::parse_from_str(raw, "%a %b %e %H:%M:%S %Y")
        .with_context(|| format!("cannot parse ps lstart {raw:?}"))?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .ok_or_else(|| anyhow!("ps lstart {raw:?} does not exist in the local timezone"))
}

fn expected_binary() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    let expected = Path::new(&home).join(".cargo/bin/vaulty");
    std::fs::canonicalize(&expected).with_context(|| format!("expected binary {} is missing", expected.display()))
}

fn ping(command: &str) -> Result<(bool, String)> {
    let mut parts = command.split_whitespace();
    let program = parts.next().ok_or_else(|| anyhow!("--ping-command is empty"))?;
    let mut child = Command::new(program)
        .args(parts)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning ping `{command}`"))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let deadline = Instant::now() + PING_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().context("waiting for ping")? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut output = stdout.join().unwrap_or_default();
    output.push_str(&stderr.join().unwrap_or_default());
    let Some(status) = status else {
        output.push_str(&format!("\n[install_verify] ping timed out after {}s and was killed", PING_TIMEOUT.as_secs()));
        return Ok((false, output.trim().to_string()));
    };
    Ok((status.success(), output.trim().to_string()))
}

fn drain<R: Read + Send + 'static>(reader: Option<R>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut reader) = reader {
            let _ = reader.read_to_string(&mut buf);
        }
        buf
    })
}
