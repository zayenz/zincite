use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let revision = String::from_utf8_lossy(&output.stdout);
    println!("cargo:rustc-env=ZINCITE_BUILD_REVISION={}", revision.trim());

    // Git resolves these paths for ordinary checkouts and linked worktrees.
    if let Ok(output) = Command::new("git")
        .args([
            "rev-parse",
            "--git-path",
            "HEAD",
            "--git-path",
            "refs",
            "--git-path",
            "packed-refs",
        ])
        .output()
        && output.status.success()
    {
        for path in String::from_utf8_lossy(&output.stdout).lines() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
