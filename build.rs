use time::{OffsetDateTime, format_description::well_known::Iso8601};

/// build helper for generating version strings from environment
fn main() {
    let version =
        std::env::var("NLOCK_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let commit = std::env::var("NLOCK_COMMIT").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_default();
    let target = std::env::var("TARGET").unwrap_or_default();

    let date = OffsetDateTime::now_utc()
        .format(&Iso8601::DATE)
        .unwrap_or_default();

    let mut long_version = version.clone();

    let extra = [commit, date, profile, target];
    if !extra.is_empty() {
        long_version.push_str(" (");
        long_version.push_str(extra.join(" ").trim());
        long_version.push(')');
    }

    println!("cargo:rustc-env=NLOCK_VERSION={version}");
    println!("cargo:rustc-env=NLOCK_LONG_VERSION={long_version}");

    println!("cargo:rerun-if-env-changed=NLOCK_VERSION");
    println!("cargo:rerun-if-env-changed=NLOCK_COMMIT");
}
