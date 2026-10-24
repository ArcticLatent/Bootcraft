//! App icons from Flathub, downloaded on first view and cached in
//! `~/.cache/bootcraft/icons`. Third-party logos stay out of the repository;
//! offline, QML falls back to a Lucide icon for the app's section.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::bridge::push_event;

const WORKERS: usize = 6;

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("bootcraft/icons"))
}

/// Flathub IDs are reverse-DNS names; anything else must not become a path.
fn valid_flatpak_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn curl(args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("curl").args(["-fsSL", "--max-time", "20"]).args(args).output().ok()?;
    out.status.success().then_some(out.stdout)
}

/// Look the icon up in Flathub's appstream data and download it.
fn download(flatpak_id: &str, target: &PathBuf) -> Option<()> {
    let info = curl(&[&format!("https://flathub.org/api/v2/appstream/{flatpak_id}")])?;
    let info: Value = serde_json::from_slice(&info).ok()?;
    let url = info["icon"].as_str().filter(|u| u.starts_with("https://"))?;
    let bytes = curl(&[url])?;
    let temp = target.with_extension("part");
    std::fs::write(&temp, bytes).ok()?;
    std::fs::rename(&temp, target).ok()
}

/// `{"apps": [{"id": "gimp", "flatpak": "org.gimp.GIMP"}, ...]}`: emits an
/// `app-icon` event (`{id, url}`) for each icon as it becomes available.
pub fn fetch(args: &Value) -> Value {
    let Some(dir) = cache_dir() else { return Value::Null };
    if std::fs::create_dir_all(&dir).is_err() {
        return Value::Null;
    }
    let mut queue = VecDeque::new();
    for app in args["apps"].as_array().into_iter().flatten() {
        let (Some(id), Some(flatpak)) = (app["id"].as_str(), app["flatpak"].as_str()) else { continue };
        if !valid_flatpak_id(flatpak) {
            continue;
        }
        let path = dir.join(format!("{flatpak}.png"));
        if path.exists() {
            push_event("app-icon", json!({ "id": id, "url": format!("file://{}", path.display()) }));
        } else {
            queue.push_back((id.to_owned(), flatpak.to_owned(), path));
        }
    }
    let queue = Arc::new(Mutex::new(queue));
    for _ in 0..WORKERS {
        let queue = Arc::clone(&queue);
        std::thread::spawn(move || {
            while let Some((id, flatpak, path)) = queue.lock().ok().and_then(|mut q| q.pop_front()) {
                if download(&flatpak, &path).is_some() {
                    push_event("app-icon", json!({ "id": id, "url": format!("file://{}", path.display()) }));
                }
            }
        });
    }
    Value::Null
}

#[cfg(test)]
mod tests {
    use super::valid_flatpak_id;

    #[test]
    fn flatpak_ids_are_checked_before_use_as_file_names() {
        assert!(valid_flatpak_id("org.gimp.GIMP"));
        assert!(valid_flatpak_id("io.github.Faugus.faugus-launcher"));
        assert!(!valid_flatpak_id("../../etc/passwd"));
        assert!(!valid_flatpak_id(".hidden"));
        assert!(!valid_flatpak_id(""));
    }
}
