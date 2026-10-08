//! Local connection commands (`varos-cli bridge pair|agents|hosts|register|status`).
//! Pairing/revocation remain compatibility no-ops under open local trust.
use super::{audit, credentials, registry, Paths};
use crate::Error;

pub const VERBS: &[&str] = &["pair", "agents", "hosts", "register", "status"];
pub fn handles(args: &[String]) -> bool {
    args.first().is_some_and(|v| VERBS.contains(&v.as_str()))
}
const USAGE: &str = "usage:
  varos-cli bridge register claude|codex|cursor|print [--dry-run] [--replace] [--claude <path>|--codex <path>]
  varos-cli bridge pair [compatibility no-op]
  varos-cli bridge agents [compatibility no-op]
  varos-cli bridge agents audit [lines]
  varos-cli bridge hosts
  varos-cli bridge status
(pair/approve/deny and agents list/revoke are compatibility no-ops)";

fn fail(e: Error) -> String {
    format!("{}: {}", e.code, e.reason)
}
pub fn run(args: Vec<String>) -> Result<i32, String> {
    let mut it = args.into_iter();
    let verb = it.next().ok_or(USAGE)?;
    let rest: Vec<String> = it.collect();
    if verb == "register" {
        return register(rest);
    }
    if verb == "pair" || (verb == "agents" && rest.first().map(String::as_str) != Some("audit")) {
        println!("{NOT_NEEDED}");
        return Ok(0);
    }
    if verb == "status" {
        println!("trust: local user");
        return Ok(0);
    }
    let paths = Paths::resolve().map_err(fail)?;
    match verb.as_str() {
        "agents" => agents(&paths, rest),
        "hosts" if rest.is_empty() => hosts(&paths),
        _ => Err(USAGE.into()),
    }
}

fn register(rest: Vec<String>) -> Result<i32, String> {
    let mut it = rest.into_iter();
    let client = super::register::Client::parse(&it.next().ok_or(USAGE)?)?;
    let (mut dry_run, mut replace, mut program) = (false, false, None);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--dry-run" => dry_run = true,
            "--replace" => replace = true,
            "--claude" | "--codex" => {
                program = Some(std::path::PathBuf::from(it.next().ok_or("missing program path")?))
            }
            _ => return Err(format!("unknown option {flag}\n{USAGE}")),
        }
    }
    super::register::register(client, dry_run, replace, program)
}

pub const NOT_NEEDED: &str = "not needed: local agents are trusted (owner decision 2026-10-08)";
fn agents(paths: &Paths, rest: Vec<String>) -> Result<i32, String> {
    if rest.first().map(String::as_str) != Some("audit") {
        println!("{NOT_NEEDED}");
        return Ok(0);
    }
    let n = rest.get(1).map(|n| n.parse().map_err(|_| "lines must be a number")).transpose()?.unwrap_or(20usize);
    for line in audit::tail(paths, n.min(1000)) {
        println!("{line}");
    }
    Ok(0)
}

fn hosts(paths: &Paths) -> Result<i32, String> {
    println!("trust: local user");
    let scan = registry::scan(paths);
    if scan.live.is_empty() {
        println!("No running Varos found.");
    }
    for r in &scan.live {
        println!(
            "instance {}  pid {}  {}  {}  host key {}",
            r.instance_id,
            r.pid,
            r.mode,
            r.app_build,
            credentials::short(&r.host_fingerprint)
        );
    }
    for d in &scan.diagnostics {
        println!("ignored: {d}");
    }
    Ok(0)
}
