use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory")).join("../..");
    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(reference) = Command::new("git")
        .args(["symbolic-ref", "--quiet", "HEAD"])
        .current_dir(&root)
        .output()
        && reference.status.success()
    {
        let reference = String::from_utf8_lossy(&reference.stdout);
        println!("cargo:rerun-if-changed=../../.git/{}", reference.trim());
    }
    println!("cargo:rerun-if-env-changed=METINBOT_BUILD_SHA");
    let version = fs::read_to_string(root.join("VERSION")).expect("VERSION file");
    println!("cargo:rustc-env=METINBOT_VERSION={}", version.trim());
    let sha = env::var("METINBOT_BUILD_SHA")
        .ok()
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&root)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=METINBOT_COMMIT={sha}");
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| !o.stdout.is_empty());
    println!("cargo:rustc-env=METINBOT_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=METINBOT_TARGET={}",
        env::var("TARGET").expect("target")
    );
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../packaging/metinbot.manifest");
        let mut resources = winresource::WindowsResource::new();
        resources
            .set("ProductName", "MetinBot")
            .set("FileDescription", "MetinBot Rust desktop application")
            .set("ProductVersion", version.trim())
            .set("FileVersion", version.trim());
        if env::var("PROFILE").as_deref() == Ok("release") {
            resources.set_manifest_file(
                root.join("packaging/metinbot.manifest")
                    .to_str()
                    .expect("manifest UTF-8 path"),
            );
        }
        resources.compile().expect("Windows native resources");
    }
}
