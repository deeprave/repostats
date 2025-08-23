use chrono::Utc;
use std::env;
use std::fs::{metadata, File};
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("version.rs");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let cargo_toml_path = Path::new(&manifest_dir).join("Cargo.toml");

    // Check if version.rs exists and if Cargo.toml is newer
    let should_regenerate = if dest_path.exists() {
        let version_rs_modified = metadata(&dest_path).unwrap().modified().unwrap();
        let cargo_toml_modified = metadata(&cargo_toml_path).unwrap().modified().unwrap();
        cargo_toml_modified > version_rs_modified
    } else {
        true // version.rs doesn't exist, so generate it
    };

    if !should_regenerate {
        return;
    }

    let mut f = File::create(&dest_path).unwrap();

    // Read plugin API version from Cargo.toml metadata
    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path).unwrap();

    let plugin_api_version = match cargo_toml_content.parse::<toml::Table>() {
        Ok(cargo_toml) => cargo_toml
            .get("package")
            .and_then(|p| p.as_table())
            .and_then(|p| p.get("metadata"))
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("plugin_api_version"))
            .and_then(|v| v.as_integer())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };
    let build_time = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    #[allow(clippy::uninlined_format_args)]
    writeln!(
        &mut f,
        r###"pub const PLUGIN_API_VERSION: &str = "{}";
pub const BUILD_TIME: &str = "{}";
pub const GIT_HASH: &str = "{}";"###,
        plugin_api_version, build_time, git_hash
    )
    .unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
