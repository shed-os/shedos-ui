use anyhow::Result;

mod recovery;
mod render;
mod slides;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    // --recovery shows only the recovery-key slide (the in-place re-trigger); the
    // normal first-run tour prepends it when a key is stashed.
    let recovery_only = std::env::args().any(|a| a == "--recovery");
    render::run(recovery_only)
}
