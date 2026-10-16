use std::process::Command;
fn main() {
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    if let Ok(result) = Command::new("git")
        .args(["-C", &root, "rev-parse", "HEAD"])
        .output()
    {
        let revision = String::from_utf8_lossy(&result.stdout).trim().to_owned();
        if result.status.success()
            && revision.len() == 40
            && revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            println!("cargo:rustc-env=MEGAMAIL_SOURCE_REVISION={revision}");
        }
    }
    let mut paths = vec!["HEAD".to_owned()];
    if let Ok(result) = Command::new("git")
        .args(["-C", &root, "symbolic-ref", "-q", "HEAD"])
        .output()
    {
        if result.status.success() {
            paths.push(String::from_utf8_lossy(&result.stdout).trim().to_owned());
        }
    }
    for path in &paths {
        if let Ok(result) = Command::new("git")
            .args(["-C", &root, "rev-parse", "--git-path", path.as_str()])
            .output()
        {
            if result.status.success() {
                println!(
                    "cargo:rerun-if-changed={}",
                    String::from_utf8_lossy(&result.stdout).trim()
                );
            }
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
