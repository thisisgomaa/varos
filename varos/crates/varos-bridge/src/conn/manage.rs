//! Owner terminal commands (`varos-cli bridge pair|agents|hosts|register`). This is the
//! temporary C1 owner channel until the pairing UI exists (mockups first — ADR-0011 §3).
use super::{audit, credentials, registry, trust, Paths};
use crate::Error;
use std::io::{BufRead, IsTerminal, Write};

pub const VERBS: &[&str] = &["pair", "agents", "hosts", "register"];
pub fn handles(args: &[String]) -> bool {
    args.first().is_some_and(|v| VERBS.contains(&v.as_str()))
}
const USAGE: &str = "usage:
  varos-cli bridge register claude|codex|cursor|print [--dry-run] [--replace] [--claude <path>|--codex <path>]
  varos-cli bridge pair [list]
  varos-cli bridge pair --approve <request-id> [--scopes read,edit[,destructive][,history]]
  varos-cli bridge pair --deny <request-id>
  varos-cli bridge agents [list]
  varos-cli bridge agents revoke <profile-id>
  varos-cli bridge agents audit [lines]
  varos-cli bridge hosts";

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
    let paths = Paths::resolve().map_err(fail)?;
    match verb.as_str() {
        "pair" => pair(&paths, rest),
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

fn pair(paths: &Paths, rest: Vec<String>) -> Result<i32, String> {
    let mut it = rest.into_iter();
    match it.next().as_deref() {
        None | Some("list") | Some("--list") => {
            let pending = trust::pending(paths).map_err(fail)?;
            if pending.is_empty() {
                println!("No agent is waiting for approval.");
            }
            for r in pending {
                print_request(&r);
            }
            Ok(0)
        }
        Some("--approve") => {
            let id = it.next().ok_or("missing request id")?;
            let mut scopes = trust::Scopes::DEFAULT_REQUEST;
            while let Some(flag) = it.next() {
                match flag.as_str() {
                    "--scopes" => scopes = trust::Scopes::parse(&it.next().ok_or("missing scopes")?).map_err(fail)?,
                    _ => return Err(format!("unknown option {flag}\n{USAGE}")),
                }
            }
            let request = trust::find_pending(paths, &id).map_err(fail)?;
            if !std::io::stdin().is_terminal() {
                return Err(
                    "approval must be typed by the owner in an interactive Terminal; agents cannot approve themselves"
                        .into(),
                );
            }
            print_request(&request);
            // The code is shown only to the agent (in its pairing_required answer), never here:
            // typing it proves the owner is approving the same agent they are looking at.
            eprint!(
                "To grant {} to this agent on this Mac, type the match code the agent showed you (like ABC-123): ",
                scopes.names()
            );
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer).map_err(|e| e.to_string())?;
            let approved = match approve(paths, &id, answer.trim(), scopes) {
                Ok(a) => a,
                Err(e) if e.code == "match_code_mismatch" => {
                    println!("That code does not match this request. Not approved; nothing changed.");
                    return Ok(1);
                }
                Err(e) => return Err(fail(e)),
            };
            println!(
                "Approved agent {} ({}) with {}. It can retry its call now. Revoke any time: varos-cli bridge agents revoke {}",
                approved.profile_id,
                approved.label,
                approved.scopes.names(),
                approved.profile_id
            );
            Ok(0)
        }
        Some("--deny") => {
            let id = it.next().ok_or("missing request id")?;
            let request = trust::find_pending(paths, &id).map_err(fail)?;
            trust::remove_pending(paths, &id);
            let _ = audit::append(paths, &audit::Entry::event("pairing_denied", &request.profile_id, "denied"));
            println!("Denied. The agent gets no access.");
            Ok(0)
        }
        _ => Err(USAGE.into()),
    }
}
fn print_request(r: &trust::PairingRequest) {
    println!(
        "request {}\n  agent label (claimed, unverified): {}\n  agent key fingerprint (verified by signature): {}\n  profile {}  asks for {}  expires in {} s",
        r.request_id,
        r.label,
        credentials::short(&r.fingerprint),
        r.profile_id,
        r.requested.names(),
        r.expires.saturating_sub(super::now_secs())
    );
}
/// Library form of the owner approval: the typed match code must equal the one the agent was
/// shown (case and spacing ignored). The CLI adds the interactive-terminal requirement.
pub fn approve(
    paths: &Paths,
    request_id: &str,
    typed_code: &str,
    scopes: trust::Scopes,
) -> Result<trust::Approved, Error> {
    let request = trust::find_pending(paths, request_id)?;
    let norm = |c: &str| c.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    if norm(typed_code).is_empty() || norm(typed_code) != norm(&request.match_code) {
        return Err(Error::new("match_code_mismatch", "the typed match code does not match this pairing request"));
    }
    let approved = trust::approve(paths, &request, scopes)?;
    trust::remove_pending(paths, request_id);
    let _ =
        audit::append(paths, &audit::Entry::event("pairing_approved", &approved.profile_id, &approved.scopes.names()));
    Ok(approved)
}
/// Library form of revocation.
pub fn revoke(paths: &Paths, profile_id: &str) -> Result<trust::Revoked, Error> {
    let revoked = trust::revoke(paths, profile_id)?;
    let _ = audit::append(paths, &audit::Entry::event("agent_revoked", profile_id, "revoked"));
    Ok(revoked)
}

fn agents(paths: &Paths, rest: Vec<String>) -> Result<i32, String> {
    let mut it = rest.into_iter();
    match it.next().as_deref() {
        None | Some("list") => {
            let file = trust::TrustFile::load(paths).map_err(fail)?;
            if file.approved.is_empty() {
                println!("No approved agents.");
            }
            for a in &file.approved {
                println!(
                    "{}  {}  scopes {}  key {}",
                    a.profile_id,
                    a.label,
                    a.scopes.names(),
                    credentials::short(&a.fingerprint)
                );
            }
            if !file.revoked.is_empty() {
                println!(
                    "revoked: {}",
                    file.revoked.iter().map(|r| r.profile_id.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
            println!("trust generation {}", file.generation);
            Ok(0)
        }
        Some("revoke") => {
            let id = it.next().ok_or("missing profile id")?;
            revoke(paths, &id).map_err(fail)?;
            println!("Revoked {id}. Its next call is refused; it would have to be paired again as a new agent.");
            Ok(0)
        }
        Some("audit") => {
            let n = it.next().map(|n| n.parse().map_err(|_| "lines must be a number")).transpose()?.unwrap_or(20usize);
            for line in audit::tail(paths, n.min(1000)) {
                println!("{line}");
            }
            Ok(0)
        }
        _ => Err(USAGE.into()),
    }
}

fn hosts(paths: &Paths) -> Result<i32, String> {
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
