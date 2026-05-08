// shedos-greeter — greetd greeter. The pixel-level rendering lives
// in `shedos-prompt-ui`; this binary owns Wayland surface lifecycle,
// keyboard handling, and the greetd IPC handshake.

mod greetd;
mod render;
mod user;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    render::run()
}
