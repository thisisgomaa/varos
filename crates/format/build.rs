// Varos: one build-time identity for UI, files and protocol surfaces.
fn main() {
    println!("cargo::rerun-if-env-changed=VAROS_PRODUCT_VERSION");
    // Worktrees have a .git file: ask git for the actual metadata paths.
    for name in ["HEAD", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]).filter(|p| std::path::Path::new(p).exists()) {
            println!("cargo::rerun-if-changed={path}");
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(&["rev-parse", "--git-path", &reference]).filter(|p| std::path::Path::new(p).exists())
    {
        println!("cargo::rerun-if-changed={path}");
    }
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let engine = env!("CARGO_PKG_VERSION");
    let product = std::env::var("VAROS_PRODUCT_VERSION").unwrap_or_else(|_| engine.into());
    println!("cargo::rustc-env=VAROS_IDENTITY=Varos {product} (engine VectorCraft {engine}, build {sha})");
}

fn git(args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}
