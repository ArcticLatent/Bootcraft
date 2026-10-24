//! Bootcraft updating itself, as ComfyPilot does. Each release publishes a
//! signed `linux-release.json` (the release workflow) naming its files with
//! their SHA-256; Bootcraft checks the signature against Arctic Latent's key
//! before trusting any of it.
//!
//! - An AppImage downloads its successor and puts it in place of the running
//!   file, without a password.
//! - The .deb, .rpm and Arch packages are installed by the helper through
//!   pkexec (`bootcraft-helper self-update`), which fetches and checks the
//!   manifest again as root rather than trusting the app.
//!
//! Either way the app starts again once it has quit.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use bootcraft_core::release::{self, ARCTIC_RELEASE_PUBLIC_KEY, BOOTCRAFT_MANIFEST, Manifest};
use serde_json::{Value, json};

use crate::bridge::push_event;
use crate::helper::INSTALLED_HELPER;

/// How this copy was installed, which decides how it updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    AppImage,
    Package,
    /// A development build: the release page instead.
    Manual,
}

impl Kind {
    fn current() -> Self {
        if running_appimage().is_some() {
            return Self::AppImage;
        }
        let exe = std::env::current_exe().ok().and_then(|exe| exe.canonicalize().ok());
        if exe.as_deref() == Some(Path::new("/usr/bin/bootcraft")) && Path::new(INSTALLED_HELPER).exists() {
            Self::Package
        } else {
            Self::Manual
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::AppImage => "appimage",
            Self::Package => "package",
            Self::Manual => "manual",
        }
    }
}

/// `{available, version, current, kind, page}`.
pub fn check() -> Result<Value, String> {
    let manifest = fetch_manifest()?;
    let current = env!("CARGO_PKG_VERSION");
    Ok(json!({
        "available": release::is_newer(&manifest.version, current),
        "version": manifest.version,
        "current": current,
        "kind": Kind::current().name(),
        "page": format!("https://github.com/{}/releases/tag/{}", manifest.repository, manifest.tag),
    }))
}

/// Install the newer release. An AppImage replies `{restart: true}` once it
/// is in place; a package replies at once and reports through `update-event`
/// and `update-exit` (see `helper::start_self_update`).
pub fn install() -> Result<Value, String> {
    match Kind::current() {
        Kind::AppImage => update_appimage(),
        Kind::Package => crate::helper::start_self_update(),
        Kind::Manual => Err("This copy of Bootcraft wasn't installed from a package or an AppImage; \
                             download the new version from the release page."
            .into()),
    }
}

/// The manifest at `BOOTCRAFT_UPDATE_MANIFEST_URL` (for testing; still
/// signature-checked) or the latest release's.
fn fetch_manifest() -> Result<Manifest, String> {
    let url = std::env::var("BOOTCRAFT_UPDATE_MANIFEST_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| BOOTCRAFT_MANIFEST.to_owned());
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", &url])
        .output()
        .map_err(|e| format!("could not run curl: {e}"))?;
    // curl -f exits with 22 for an HTTP error, e.g. before the first release.
    match out.status.code() {
        Some(0) => {}
        Some(22) => return Err("No Bootcraft release was found on GitHub.".into()),
        _ => return Err("Couldn't reach GitHub to check for updates.".into()),
    }
    release::verify(&String::from_utf8_lossy(&out.stdout), ARCTIC_RELEASE_PUBLIC_KEY)
}

fn progress(line: &str) {
    push_event("update-event", json!({ "event": "output", "line": line }));
}

fn update_appimage() -> Result<Value, String> {
    let target = running_appimage().ok_or("this copy isn't running as an AppImage")?;
    let manifest = fetch_manifest()?;
    if !release::is_newer(&manifest.version, env!("CARGO_PKG_VERSION")) {
        return Err("You already have the latest version.".into());
    }
    let asset = release::appimage_asset(&manifest)
        .ok_or_else(|| format!("release {} has no AppImage", manifest.version))?;
    progress(&format!("Release {}: signature verified.", manifest.version));

    let dir = cache_dir().ok_or("no cache folder")?.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let downloaded = dir.join(&asset.name);
    progress(&format!("Downloading {}…", asset.name));
    let status = Command::new("curl")
        .args(["-fsSL", "--max-time", "1800", "-o"])
        .arg(&downloaded)
        .arg(&asset.download_url)
        .status()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if !status.success() {
        return Err(format!("Couldn't download {}.", asset.name));
    }
    let digest = release::sha256_file(&downloaded).map_err(|e| e.to_string())?;
    if !digest.eq_ignore_ascii_case(&asset.sha256) {
        let _ = std::fs::remove_file(&downloaded);
        return Err(format!("{} doesn't match the signed checksum; it was deleted.", asset.name));
    }
    progress(&format!("{}: checksum verified.", asset.name));
    replace_appimage(&downloaded, &target)?;
    progress(&format!("Bootcraft {} is installed; restarting.", manifest.version));
    relaunch_after_exit()?;
    Ok(json!({ "restart": true, "version": manifest.version }))
}

/// The AppImage file this process runs from (`$APPIMAGE`, set by the
/// AppImage runtime), if it is one.
fn running_appimage() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    path.is_file().then_some(path)
}

fn cache_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .map(|dir| dir.join("bootcraft"))
}

/// Puts the downloaded AppImage in place of the running one: a copy next to
/// it, made executable, then renamed over it (the running copy keeps its
/// open file, so this is safe while it runs).
fn replace_appimage(downloaded: &Path, target: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let name = target.file_name().ok_or("the AppImage has no file name")?.to_string_lossy();
    let staged = target.with_file_name(format!(".{name}.update"));
    std::fs::copy(downloaded, &staged)
        .map_err(|e| format!("Can't write next to {} (is its folder writable?): {e}", target.display()))?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    if let Err(err) = std::fs::rename(&staged, target) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Couldn't replace {}: {err}", target.display()));
    }
    let _ = std::fs::remove_file(downloaded);
    Ok(())
}

/// Starts the app again once this process has exited: a detached shell
/// waits for this PID, then runs the AppImage or the installed binary.
pub fn relaunch_after_exit() -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let program = match running_appimage() {
        Some(appimage) => appimage,
        None => {
            // A package upgrade replaced the binary: /proc/self/exe then
            // reads "<path> (deleted)"; the path itself has the new one.
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let exe = exe.to_string_lossy();
            PathBuf::from(exe.strip_suffix(" (deleted)").unwrap_or(&exe))
        }
    };
    Command::new("sh")
        .arg("-c")
        .arg(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.3; done; exec "$2""#)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(&program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
        .map_err(|e| format!("Couldn't schedule a restart of {}: {e}", program.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_an_appimage_swaps_the_file_and_makes_it_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("bootcraft-appimage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("Bootcraft.AppImage");
        let downloaded = dir.join("Bootcraft-2.0.0-x86_64.AppImage");
        std::fs::write(&target, "old").unwrap();
        std::fs::write(&downloaded, "new").unwrap();

        replace_appimage(&downloaded, &target).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(!downloaded.exists());
        assert!(!dir.join(".Bootcraft.AppImage.update").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
