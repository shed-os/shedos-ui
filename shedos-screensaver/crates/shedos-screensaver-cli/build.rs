// PKGBUILD passes pkgver-pkgrel via SHEDOS_VERSION so `--version`
// matches what pacman reports. Plain `cargo build` falls back to
// the crate's Cargo.toml SemVer for dev runs.

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
