// shedos-greeter — greetd greeter that mirrors the hyprlock visual.
// Hands off to the Wayland render module; subsequent commits add the
// password input, greetd IPC, clock + branding text, and theme loading.

mod greetd;
mod render;
mod text;
mod user;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Yield DRM to cage. Best-effort; silent no-op if plymouthd isn't running.
    let _ = std::process::Command::new("plymouth").arg("deactivate").status();

    // For now, take the wallpaper as positional argv[1]; the theme
    // reconciler will eventually feed this via /etc/shedos/themes/current/.
    let wallpaper = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/share/shedos/wallpapers/dusk-blurred.png"));

    render::run(&wallpaper)
}
