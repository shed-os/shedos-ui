// shedos-greeter binary. Rendering goes through `shedos-prompt-ui`;
// this binary owns the Wayland surface, keyboard, and greetd IPC.

mod greetd;
mod render;
mod user;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    render::run()
}
