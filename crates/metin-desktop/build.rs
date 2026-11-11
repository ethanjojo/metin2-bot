use std::{env, fs, path::PathBuf, process::Command};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AppIdentity {
    app_id: String,
    product_name: String,
    publisher: String,
    description: String,
    internal_name: String,
    original_filename: String,
    copyright: String,
}
fn main() {
    let root =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory")).join("../..");
    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-changed=../../packaging/app-identity.json");
    let identity: AppIdentity = serde_json::from_str(
        &fs::read_to_string(root.join("packaging/app-identity.json")).expect("app identity file"),
    )
    .expect("valid app identity");
    assert!(
        identity.app_id.split('.').count() >= 3
            && identity
                .app_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'.'),
        "invalid application id"
    );
    println!("cargo:rustc-env=COMPANION_APP_ID={}", identity.app_id);
    println!("cargo:rustc-env=COMPANION_PUBLISHER={}", identity.publisher);
    println!(
        "cargo:rustc-env=COMPANION_PRODUCT_NAME={}",
        identity.product_name
    );
    for value in [
        &identity.product_name,
        &identity.publisher,
        &identity.description,
        &identity.internal_name,
        &identity.original_filename,
        &identity.copyright,
    ] {
        assert!(
            !value.trim().is_empty() && !value.contains(['\n', '\r', '\0']),
            "invalid app identity value"
        );
    }
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
    println!("cargo:rerun-if-env-changed=COMPANION_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=COMPANION_BUILD_NUMBER");
    println!("cargo:rerun-if-changed=../../.git/index");
    let base = fs::read_to_string(root.join("VERSION")).expect("VERSION file");
    let version = env::var("COMPANION_BUILD_VERSION").unwrap_or_else(|_| base.trim().into());
    let parsed = semver::Version::parse(&version).expect("valid release SemVer");
    let base = semver::Version::parse(base.trim()).expect("valid base version");
    assert_eq!(
        base.to_string(),
        env::var("CARGO_PKG_VERSION").expect("Cargo version"),
        "VERSION and Cargo workspace version must agree"
    );
    assert_eq!(
        (parsed.major, parsed.minor, parsed.patch),
        (base.major, base.minor, base.patch),
        "release version must match VERSION core"
    );
    let build_number: u16 = env::var("COMPANION_BUILD_NUMBER")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .expect("Windows build number 0..65535");
    assert!(
        [parsed.major, parsed.minor, parsed.patch]
            .into_iter()
            .all(|n| n <= u16::MAX as u64),
        "Windows version component overflow"
    );
    let numeric = format!(
        "{}.{}.{}.{}",
        parsed.major, parsed.minor, parsed.patch, build_number
    );
    println!("cargo:rustc-env=COMPANION_VERSION={}", version.trim());
    let sha = {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    }
    .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=COMPANION_COMMIT={sha}");
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| !o.stdout.is_empty());
    println!("cargo:rustc-env=COMPANION_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=COMPANION_TARGET={}",
        env::var("TARGET").expect("target")
    );
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../packaging/companion.manifest");
        println!("cargo:rerun-if-changed=../../packaging/icons/companion.ico");
        let mut resources = winresource::WindowsResource::new();
        resources
            .set("ProductName", &identity.product_name)
            .set("CompanyName", &identity.publisher)
            .set("FileDescription", &identity.description)
            .set("InternalName", &identity.internal_name)
            .set("OriginalFilename", &identity.original_filename)
            .set("LegalCopyright", &identity.copyright)
            .set("ProductVersion", version.trim())
            .set("FileVersion", &numeric)
            .set("Comments", &format!("Commit {sha}"))
            .set("PrivateBuild", if dirty { "dirty" } else { "clean" });
        let parts = [
            parsed.major,
            parsed.minor,
            parsed.patch,
            u64::from(build_number),
        ];
        let packed = parts
            .into_iter()
            .fold(0u64, |value, part| (value << 16) | part);
        resources.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
        resources.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
        resources.set_icon(
            root.join("packaging/icons/companion.ico")
                .to_str()
                .expect("icon path"),
        );
        if env::var("PROFILE").as_deref() == Ok("release") {
            resources.set_manifest_file(
                root.join("packaging/companion.manifest")
                    .to_str()
                    .expect("manifest UTF-8 path"),
            );
        }
        resources.compile().expect("Windows native resources");
    }
    #[cfg(not(windows))]
    let _ = numeric;
}
