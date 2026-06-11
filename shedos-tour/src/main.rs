use anyhow::Result;

mod render;
mod slides;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    render::run()
}
