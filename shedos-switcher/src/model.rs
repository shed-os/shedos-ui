use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Client {
    address: String,
    mapped: bool,
    title: String,
    class: String,
    workspace: Ws,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: i64,
}

#[derive(Deserialize)]
struct Ws {
    id: i64,
}

pub struct Window {
    pub address: String,
    pub title: String,
    pub class: String,
    pub workspace: i64,
    /// 64×64 BGRA premultiplied-friendly pixels, when an icon resolved.
    pub icon: Option<Vec<u8>>,
}

pub const ICON_PX: u32 = 64;

/// Mapped windows in most-recently-used order (focusHistoryID 0 is
/// the currently focused window).
pub fn list_windows() -> Result<Vec<Window>> {
    let out = Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .context("running hyprctl clients")?;
    let mut clients: Vec<Client> =
        serde_json::from_slice(&out.stdout).context("parsing hyprctl clients")?;
    clients.retain(|c| c.mapped && !c.title.is_empty());
    clients.sort_by_key(|c| c.focus_history_id);
    Ok(clients
        .into_iter()
        .map(|c| Window {
            icon: resolve_icon(&c.class),
            address: c.address,
            title: c.title,
            class: c.class,
            workspace: c.workspace.id,
        })
        .collect())
}

pub fn focus(address: &str) {
    let out = Command::new("hyprctl")
        // ShedOS's Hyprland Lua layer evaluates dispatch args as Lua;
        // the legacy `focuswindow address:…` form is a syntax error there.
        .args([
            "dispatch",
            &format!("hl.dsp.focus({{ window = \"address:{address}\" }})"),
        ])
        .output();
    match out {
        // The Lua layer reports errors on stdout with rc 0; surface them.
        Ok(o) if String::from_utf8_lossy(&o.stdout).contains("error") => {
            log::warn!("focus dispatch: {}", String::from_utf8_lossy(&o.stdout).trim());
        }
        Ok(_) => {}
        Err(e) => log::warn!("hyprctl spawn failed: {e}"),
    }
}

/// Class → desktop entry → Icon= name → a PNG from the usual places,
/// scaled to ICON_PX. None means the caller draws a letter tile —
/// a deliberate look, not a broken-image glyph.
fn resolve_icon(class: &str) -> Option<Vec<u8>> {
    let icon_name = desktop_icon_name(class)?;
    let path = find_icon_png(&icon_name)?;
    let img = image::open(&path).ok()?;
    let img = img.resize_exact(ICON_PX, ICON_PX, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8();
    // Canvas format is ARGB8888 little-endian = BGRA bytes.
    let mut bgra = rgba.into_raw();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(bgra)
}

fn desktop_icon_name(class: &str) -> Option<String> {
    let lower = class.to_lowercase();
    let dirs = ["/usr/share/applications", "/usr/local/share/applications"];
    // Pass 1: file stem matches the class (covers kitty, code,
    // google-chrome, most apps).
    for dir in dirs {
        let p = PathBuf::from(dir).join(format!("{lower}.desktop"));
        if let Some(name) = icon_field(&p) {
            return Some(name);
        }
    }
    // Pass 2: scan for a StartupWMClass match.
    for dir in dirs {
        let entries = std::fs::read_dir(dir).ok()?;
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let wm = text
                .lines()
                .find_map(|l| l.strip_prefix("StartupWMClass="))
                .map(str::trim);
            if wm.is_some_and(|w| w.eq_ignore_ascii_case(class)) {
                if let Some(name) = text
                    .lines()
                    .find_map(|l| l.strip_prefix("Icon="))
                    .map(|s| s.trim().to_string())
                {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn icon_field(path: &PathBuf) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("Icon="))
        .map(|s| s.trim().to_string())
}

fn find_icon_png(name: &str) -> Option<PathBuf> {
    if name.starts_with('/') {
        let p = PathBuf::from(name);
        return p.exists().then_some(p);
    }
    let sizes = [
        "128x128", "256x256", "512x512", "96x96", "64x64", "48x48", "scalable",
    ];
    let themes = ["hicolor", "Papirus", "Adwaita", "breeze"];
    for theme in themes {
        for size in sizes {
            let p = PathBuf::from(format!(
                "/usr/share/icons/{theme}/{size}/apps/{name}.png"
            ));
            if p.exists() {
                return Some(p);
            }
        }
    }
    let pixmap = PathBuf::from(format!("/usr/share/pixmaps/{name}.png"));
    pixmap.exists().then_some(pixmap)
}
