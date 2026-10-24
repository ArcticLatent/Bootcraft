//! How much the selected apps will download, for the Start setup card.
//!
//! System packages are sized by the distribution's own tools (pacman includes
//! the dependencies that aren't installed yet; dnf and apt-cache only the
//! packages themselves), Flatpaks by Flathub's API (without shared runtimes).
//! AUR builds and signed releases aren't sized, so the total is marked partial.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use bootcraft_core::AppPick;
use bootcraft_core::apps::{Preference, Source, Via};
use bootcraft_core::catalog::catalog;
use bootcraft_core::probe::{Probe, Status};
use bootcraft_core::system::{Distro, System};
use serde_json::{Value, json};

/// Flathub download sizes, which don't change while the app runs.
static FLATHUB_SIZES: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

pub fn download_size(sys: &System, args: &Value) -> Value {
    let picks: Vec<AppPick> = serde_json::from_value(args["apps"].clone()).unwrap_or_default();
    let preference: Preference = serde_json::from_value(args["preference"].clone()).unwrap_or_default();
    let Some(distro) = sys.distro() else { return json!({ "bytes": 0, "partial": true }) };
    let catalog = catalog(sys);
    let probe = Probe::new(Path::new("/"), distro);

    let mut bytes = 0u64;
    let mut partial = distro != Distro::Arch;
    let mut native = Vec::new();
    for pick in &picks {
        let Some(app) = catalog.app(&pick.id) else { continue };
        let Some(choice) = app.choose(sys, preference, pick.source) else { continue };
        let Some(steps) = app.steps(choice.source) else { continue };
        if probe.status(&steps) == Status::Done {
            continue;
        }
        match (choice.source, &app.native) {
            (Source::Flatpak, _) => match app.flatpak.as_ref().and_then(|f| flathub_size(&f.id)) {
                Some(size) => bytes += size,
                None => partial = true,
            },
            (Source::Native, Some(n)) if matches!(n.via, Via::Repo | Via::RpmFusion) => {
                native.extend(n.packages.iter().filter(|p| probe.is_installed(p) != Some(true)).cloned());
            }
            _ => partial = true,
        }
    }
    native.sort();
    native.dedup();
    match native_size(distro, &native) {
        Some(size) => bytes += size,
        None if !native.is_empty() => partial = true,
        None => {}
    }
    json!({ "bytes": bytes, "partial": partial })
}

fn flathub_size(id: &str) -> Option<u64> {
    if let Some(size) = FLATHUB_SIZES.lock().ok()?.as_ref().and_then(|m| m.get(id).copied()) {
        return Some(size);
    }
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "15", &format!("https://flathub.org/api/v2/summary/{id}")])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let size = serde_json::from_slice::<Value>(&out.stdout).ok()?["download_size"].as_u64()?;
    FLATHUB_SIZES.lock().ok()?.get_or_insert_with(HashMap::new).insert(id.to_owned(), size);
    Some(size)
}

fn output(argv: &[&str]) -> Option<String> {
    let out = Command::new(argv[0]).args(&argv[1..]).env("LC_ALL", "C").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn native_size(distro: Distro, packages: &[String]) -> Option<u64> {
    if packages.is_empty() {
        return None;
    }
    let names: Vec<&str> = packages.iter().map(String::as_str).filter(|p| !p.starts_with('@')).collect();
    match distro {
        // Includes the dependencies that aren't installed yet.
        Distro::Arch => {
            let argv: Vec<&str> =
                ["pacman", "-Sp", "--needed", "--print-format", "%s"].into_iter().chain(names).collect();
            output(&argv).map(|text| sum_lines(&text, ""))
        }
        Distro::Fedora => {
            let argv: Vec<&str> =
                ["dnf", "repoquery", "-q", "--latest-limit", "1", "--qf", "%{downloadsize}\\n"]
                    .into_iter()
                    .chain(names)
                    .collect();
            output(&argv).map(|text| sum_lines(&text, ""))
        }
        Distro::Debian | Distro::Ubuntu | Distro::Mint => {
            let argv: Vec<&str> =
                ["apt-cache", "show", "--no-all-versions"].into_iter().chain(names).collect();
            output(&argv).map(|text| sum_lines(&text, "Size:"))
        }
    }
}

/// The sum of the numbers on lines starting with `prefix` ("" for every line).
fn sum_lines(text: &str, prefix: &str) -> u64 {
    text.lines().filter_map(|l| l.strip_prefix(prefix)).filter_map(|v| v.trim().parse::<u64>().ok()).sum()
}

#[cfg(test)]
mod tests {
    use super::sum_lines;

    #[test]
    fn sums() {
        assert_eq!(sum_lines("2190717\n146854\n", ""), 2_337_571);
        assert_eq!(
            sum_lines("Package: a\nSize: 100\nPackage: b\nSize: 20\nInstalled-Size: 9\n", "Size:"),
            120
        );
    }
}
