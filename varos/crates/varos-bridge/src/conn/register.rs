//! One-time, user-scope agent registration (ADR-0011 §4). Registration carries only the
//! executable path and `mcp` — never a token, socket, epoch or private key.
//! Syntax checked 2026-10-07: Claude Code docs (code.claude.com/docs/en/mcp: `claude mcp add
//! [options] <name> -- <command> [args...]`, `--scope user`, `--transport stdio`) and the local
//! `codex mcp add --help` (`codex mcp add [OPTIONS] <NAME> -- <COMMAND>...`).
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const SERVER_NAME: &str = "varos";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Claude,
    Codex,
    Cursor,
    Print,
}
impl Client {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "cursor" => Ok(Self::Cursor),
            "print" => Ok(Self::Print),
            other => Err(format!("unknown client {other:?}; use claude, codex, cursor or print")),
        }
    }
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}
/// `varos-bridge` next to the running `varos-cli` (both live in `Varos.app/Contents/MacOS/`).
pub fn bridge_path() -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().and_then(|p| p.canonicalize()).map_err(|e| format!("cannot locate varos-cli: {e}"))?;
    let dir = exe.parent().ok_or("varos-cli has no parent directory")?;
    let bridge = dir.join(exe_name("varos-bridge"));
    if !bridge.is_file() {
        return Err(format!(
            "varos-bridge was not found next to varos-cli ({}). Build it (cargo build -p varos-bridge) or use the copy inside Varos.app.",
            bridge.display()
        ));
    }
    Ok(bridge)
}
/// Paths that will not survive (Cargo output, worktrees) get an honest warning.
pub fn location_warning(path: &Path) -> Option<String> {
    let text = path.to_string_lossy();
    (text.contains("/target/") || text.contains("/.claude/worktrees/")).then(|| {
        format!(
            "warning: {} is a build-folder path, not the installed app. It works until that folder is rebuilt or removed; re-run register from Varos.app/Contents/MacOS/varos-cli once Varos is installed with the bundled helper.",
            path.display()
        )
    })
}
pub fn claude_args(bridge: &Path) -> Vec<String> {
    ["mcp", "add", "--transport", "stdio", "--scope", "user", SERVER_NAME, "--"]
        .iter()
        .map(|s| s.to_string())
        .chain([bridge.to_string_lossy().into_owned(), "mcp".into()])
        .collect()
}
pub fn claude_remove_args() -> Vec<String> {
    ["mcp", "remove", "--scope", "user", SERVER_NAME].iter().map(|s| s.to_string()).collect()
}
pub fn codex_args(bridge: &Path) -> Vec<String> {
    ["mcp", "add", SERVER_NAME, "--"]
        .iter()
        .map(|s| s.to_string())
        .chain([bridge.to_string_lossy().into_owned(), "mcp".into()])
        .collect()
}
pub fn codex_remove_args() -> Vec<String> {
    ["mcp", "remove", SERVER_NAME].iter().map(|s| s.to_string()).collect()
}
/// Generic `mcpServers` entry (merge only this entry; Cursor: `~/.cursor/mcp.json`).
pub fn mcp_json(bridge: &Path) -> Value {
    json!({"mcpServers":{SERVER_NAME:{"command":bridge.to_string_lossy(),"args":["mcp"]}}})
}
pub fn shell_quote(text: &str) -> String {
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || "/._-=:".contains(c)) {
        text.into()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}
pub fn command_line(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string()).chain(args.iter().map(|a| shell_quote(a))).collect::<Vec<_>>().join(" ")
}
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(exe_name(name))).find(|p| p.is_file())
}
/// Claude Code desktop ships its CLI inside its support folder; offered as a hint only.
pub fn desktop_bundled_claude() -> Option<PathBuf> {
    let home = super::fsutil::user_home_dir().ok()?;
    let root = home.join("Library/Application Support/Claude/claude-code");
    let mut versions: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let key: Vec<u64> = name.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
            let bin = e.path().join("claude.app/Contents/MacOS/claude");
            bin.is_file().then_some((key, bin))
        })
        .collect();
    versions.sort();
    versions.pop().map(|(_, p)| p)
}

fn run_tool(program: &Path, args: &[String]) -> Result<(bool, String), String> {
    let out = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("could not run {}: {e}", program.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok((out.status.success(), text.trim().to_string()))
}

/// Register (or print) the user-scope server entry. Returns the process exit code.
pub fn register(client: Client, dry_run: bool, replace: bool, program: Option<PathBuf>) -> Result<i32, String> {
    let bridge = bridge_path()?;
    if let Some(w) = location_warning(&bridge) {
        eprintln!("{w}");
    }
    let (name, args, remove) = match client {
        Client::Claude => ("claude", claude_args(&bridge), claude_remove_args()),
        Client::Codex => ("codex", codex_args(&bridge), codex_remove_args()),
        Client::Cursor | Client::Print => {
            if client == Client::Print {
                println!("Claude Code (once, any folder):\n  {}", command_line("claude", &claude_args(&bridge)));
                println!("Codex (once):\n  {}", command_line("codex", &codex_args(&bridge)));
                println!("Cursor / other mcpServers clients — merge this entry into the user-level file (Cursor: ~/.cursor/mcp.json):");
            } else {
                println!("Merge this entry into ~/.cursor/mcp.json (keep your other servers):");
            }
            println!("{}", serde_json::to_string_pretty(&mcp_json(&bridge)).expect("json"));
            println!(
                "No token or secret is part of this entry. The first time, Varos asks the owner to approve the agent."
            );
            return Ok(0);
        }
    };
    let program = program.or_else(|| find_on_path(name));
    let Some(program) = program.filter(|_| !dry_run) else {
        if !dry_run {
            println!("`{name}` is not on PATH, so nothing was registered. Run this once yourself:");
        }
        println!("{}", command_line(name, &args));
        if client == Client::Claude && !dry_run {
            if let Some(bundled) = desktop_bundled_claude() {
                println!(
                    "Claude Code desktop's own CLI is at:\n  {}\nyou can run: varos-cli bridge register claude --claude {}",
                    bundled.display(),
                    shell_quote(&bundled.to_string_lossy())
                );
            }
        }
        return Ok(if dry_run { 0 } else { 2 });
    };
    let (ok, text) = run_tool(&program, &args)?;
    if ok {
        println!("Registered Varos for {name} (user scope): {}", command_line(&program.to_string_lossy(), &args));
        println!("Open a new {name} session in any folder and say \"use Varos\".");
        return Ok(0);
    }
    if text.contains("already exists") {
        if !replace {
            println!("{text}\nA Varos entry already exists. Re-run with --replace to replace only that entry.");
            return Ok(1);
        }
        let (removed, rtext) = run_tool(&program, &remove)?;
        if !removed {
            println!("{rtext}");
            return Ok(1);
        }
        let (ok, text) = run_tool(&program, &args)?;
        println!("{text}");
        return Ok(if ok { 0 } else { 1 });
    }
    println!("{text}");
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_text_is_token_free_and_uses_documented_syntax() {
        let bridge = Path::new("/Applications/Varos.app/Contents/MacOS/varos-bridge");
        assert_eq!(
            command_line("claude", &claude_args(bridge)),
            "claude mcp add --transport stdio --scope user varos -- /Applications/Varos.app/Contents/MacOS/varos-bridge mcp"
        );
        assert_eq!(
            command_line("codex", &codex_args(bridge)),
            "codex mcp add varos -- /Applications/Varos.app/Contents/MacOS/varos-bridge mcp"
        );
        let json = mcp_json(bridge).to_string();
        for text in [json, command_line("claude", &claude_args(bridge)), command_line("codex", &codex_args(bridge))] {
            for secret in ["token", "--attach", "epoch", "sock"] {
                assert!(!text.contains(secret), "{text} contains {secret}");
            }
        }
        assert_eq!(shell_quote("/Users/a b/x"), "'/Users/a b/x'");
        assert!(location_warning(Path::new("/r/varos/target/debug/varos-bridge")).is_some());
        assert!(location_warning(bridge).is_none());
    }
}
