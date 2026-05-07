// PKGBUILD passes the package's CalVer (pkgver-pkgrel) via the
// SHEDOS_VERSION env var at build time so `--version` matches what
// pacman reports. Without packaging (a plain `cargo build` from the
// workspace), fall back to the crate's SemVer from Cargo.toml so
// dev runs still produce a sensible version string.

fn main() {
    let version = std::env::var("SHEDOS_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string())
        });
    println!("cargo:rustc-env=SHEDOS_VERSION={version}");
    println!("cargo:rerun-if-env-changed=SHEDOS_VERSION");
}
