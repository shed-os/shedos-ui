use anyhow::Result;

mod render;
mod widgets;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    render::run()
}
