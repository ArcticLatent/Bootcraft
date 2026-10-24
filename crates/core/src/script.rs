//! Export a plan as a standalone bash script.
//!
//! The script does what the helper would, step by step, so a plan can be
//! kept, reviewed, or run on a machine without Bootcraft. Checks that depend
//! on the machine's state (is it installed, what is there to remove) happen
//! when the script runs. Config-file edits are written out as the finished
//! files, computed from this machine's files at export time. Signed releases
//! are resolved at export time too, and pinned to the verified checksum.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::edit::Edit;
use crate::plan::Plan;
use crate::system::Distro;
use crate::task::{APT_GET, Step, shell_quote};

/// A signed-release package resolved for this machine: download URL and
/// SHA-256 from the verified manifest.
pub struct Resolved {
    pub url: String,
    pub sha256: String,
    pub file_name: String,
}

/// Which file of a signed release a step installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseFile {
    /// The distribution's package.
    Package,
    AppImage,
}

/// Look a signed release up with curl and verify its manifest, for
/// [`to_script`]'s `resolve`.
pub fn resolve_signed(distro: Distro) -> impl FnMut(&str, &str, ReleaseFile) -> Option<Resolved> {
    move |manifest_url: &str, public_key: &str, file: ReleaseFile| {
        let out = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "30", manifest_url])
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        let manifest = crate::release::verify(&String::from_utf8_lossy(&out.stdout), public_key).ok()?;
        let asset = match file {
            ReleaseFile::Package => crate::release::asset_for(&manifest, distro)?,
            ReleaseFile::AppImage => crate::release::appimage_asset(&manifest)?,
        };
        Some(Resolved {
            url: asset.download_url.clone(),
            sha256: asset.sha256.to_ascii_lowercase(),
            file_name: asset.name.clone(),
        })
    }
}

/// `resolve` looks up a signed release (manifest URL, public key); `None`
/// leaves a failing step that says why.
pub fn to_script(
    plan: &Plan,
    root: &Path,
    exported: &str,
    resolve: &mut dyn FnMut(&str, &str, ReleaseFile) -> Option<Resolved>,
) -> String {
    let mut out = String::new();
    let distro = plan.distro;
    let _ = write!(
        out,
        r#"#!/usr/bin/env bash
# Bootcraft setup plan for {name}, exported {exported}.
#
# Run it as root: sudo bash bootcraft-setup.sh
# It stops at the first error. Config files are written as Bootcraft computed
# them from this machine's files at export time; originals are kept as
# <file>.bootcraft.bak.
set -euo pipefail
[[ $EUID -eq 0 ]] || {{ echo "Run this as root: sudo bash $0" >&2; exit 1; }}
user="${{SUDO_USER:-}}"

{helpers}
"#,
        name = distro.name(),
        helpers = helpers(distro),
    );
    for task in &plan.tasks {
        let _ = writeln!(out, "\nheading {}", shell_quote(&task.title));
        for step in &task.steps {
            out.push_str(&step_script(step, distro, root, resolve));
        }
    }
    if plan.reboot_required() {
        out.push_str("\necho\necho \"Done. Restart to finish the setup.\"\n");
    } else {
        out.push_str("\necho\necho \"Done.\"\n");
    }
    out
}

/// Shell functions the steps use, per package manager.
fn helpers(distro: Distro) -> String {
    let common = r#"heading() { echo; echo "==> $1"; }
# The first original of a file Bootcraft rewrites.
backup() { [[ -e "$1" && ! -e "$1.bootcraft.bak" ]] && cp -a "$1" "$1.bootcraft.bak" || true; }
# Installed package names matching names or prefixes ending in *.
matching() {
  local names pattern
  names="$(installed_names)"
  for pattern in "$@"; do
    if [[ "$pattern" == *'*' ]]; then grep "^${pattern%\*}" <<<"$names" || true
    else grep -x -- "$pattern" <<<"$names" || true; fi
  done
}
"#;
    let specific = match distro {
        Distro::Arch => r#"# Installed, or provided by something installed.
installed() { pacman -T "$@" >/dev/null 2>&1; }
installed_names() { pacman -Qq; }
# AUR packages, built as the invoking user with an AUR helper when there is one.
aur_install() {
  [[ -n "$user" ]] || { echo "AUR packages are built as your user; run this with sudo" >&2; return 1; }
  if command -v yay >/dev/null; then runuser -u "$user" -- yay -S --needed --noconfirm "$@"
  elif command -v paru >/dev/null; then runuser -u "$user" -- paru -S --needed --noconfirm "$@"
  else
    local pkg dir
    for pkg in "$@"; do
      dir="$(runuser -u "$user" -- mktemp -d)"
      runuser -u "$user" -- git clone --depth 1 "https://aur.archlinux.org/$pkg.git" "$dir"
      (cd "$dir" && runuser -u "$user" -- makepkg -s --noconfirm)
      pacman -U --needed --noconfirm "$dir"/*.pkg.tar.zst
    done
  fi
}
"#
        .to_owned(),
        Distro::Fedora => r#"installed() { local p; for p in "$@"; do rpm -q --whatprovides "$p" >/dev/null 2>&1 || return 1; done; }
installed_names() { rpm -qa --qf '%{NAME}\n'; }
"#
        .to_owned(),
        Distro::Debian | Distro::Ubuntu | Distro::Mint => {
            let apt = APT_GET.join(" ");
            format!(
                r#"installed() {{ local p; for p in "$@"; do dpkg-query -W -f='${{Status}}' "$p" 2>/dev/null | grep -q 'ok installed' || return 1; done; }}
installed_names() {{ dpkg-query -W -f='${{Package}} ${{Status}}\n' | awk '$NF == "installed" {{ print $1 }}'; }}
# apt-get without questions; refresh the package lists after the sources change.
apt_fresh=0
apt() {{ {apt} "$@"; }}
apt_install() {{ [[ $apt_fresh == 1 ]] || {{ apt update; apt_fresh=1; }}; apt install "$@"; }}
"#
            )
        }
    };
    format!("{common}{specific}")
}

fn quoted(items: &[String]) -> String {
    items.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
}

/// `if ! installed a || ...; then` guard for steps skipped when any of `unless` is installed.
fn guarded(unless: &[String], body: &str) -> String {
    if unless.is_empty() {
        return body.to_owned();
    }
    let checks: Vec<String> = unless.iter().map(|p| format!("installed {}", shell_quote(p))).collect();
    let body = body.trim_end().replace('\n', "\n  ");
    format!("if ! {{ {}; }}; then\n  {body}\nfi\n", checks.join(" || "))
}

fn install_command(distro: Distro, packages: &[String]) -> String {
    match distro {
        Distro::Arch => format!("pacman -S --needed --noconfirm {}\n", quoted(packages)),
        Distro::Fedora => format!("dnf -y install {}\n", quoted(packages)),
        Distro::Debian | Distro::Ubuntu | Distro::Mint => format!("apt_install {}\n", quoted(packages)),
    }
}

/// A heredoc that writes `contents` to `path`, keeping a backup.
fn write_file(path: &Path, contents: &str) -> String {
    let path_q = shell_quote(&path.to_string_lossy());
    let mut out = String::new();
    if let Some(parent) = path.parent() {
        let _ = writeln!(out, "mkdir -p {}", shell_quote(&parent.to_string_lossy()));
    }
    let _ = writeln!(out, "backup {path_q}");
    let _ = write!(out, "cat > {path_q} <<'BOOTCRAFT_EOF'\n{contents}");
    if !contents.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("BOOTCRAFT_EOF\n");
    if path.starts_with("/etc/apt") {
        out.push_str("apt_fresh=0\n");
    }
    out
}

fn edited(path: &Path, edit: &Edit, if_exists: bool, root: &Path) -> String {
    let file = root.join(path.strip_prefix("/").unwrap_or(path));
    match fs::read_to_string(&file) {
        Err(_) if if_exists => format!("# {} doesn't exist here; nothing to change.\n", path.display()),
        Ok(text) if edit.is_applied(&text) => format!("# {} already has: {edit}\n", path.display()),
        current => {
            let text = current.unwrap_or_default();
            format!("# {edit}\n{}", write_file(path, &edit.apply(&text)))
        }
    }
}

#[allow(clippy::too_many_lines)] // One arm per step kind.
fn step_script(
    step: &Step,
    distro: Distro,
    root: &Path,
    resolve: &mut dyn FnMut(&str, &str, ReleaseFile) -> Option<Resolved>,
) -> String {
    match step {
        Step::Install { packages, unless } => guarded(unless, &install_command(distro, packages)),
        Step::Remove { packages } => {
            let remove = match distro {
                Distro::Arch => "pacman -Rns --noconfirm",
                Distro::Fedora => "dnf -y remove",
                Distro::Debian | Distro::Ubuntu | Distro::Mint => "apt remove",
            };
            format!(
                "mapfile -t remove < <(matching {})\nif ((${{#remove[@]}})); then {remove} \"${{remove[@]}}\"; fi\n",
                quoted(packages)
            )
        }
        Step::AurInstall { packages, unless } => {
            guarded(unless, &format!("aur_install {}\n", quoted(packages)))
        }
        Step::SignedPackage { package, manifest_url, public_key } => {
            match resolve(manifest_url, public_key, ReleaseFile::Package) {
                Some(resolved) => {
                    let file = format!("/var/cache/bootcraft/downloads/{}", resolved.file_name);
                    let file_q = shell_quote(&file);
                    let install = match distro {
                        Distro::Arch => format!("pacman -U --needed --noconfirm {file_q}\n"),
                        Distro::Fedora => format!("dnf -y install {file_q}\n"),
                        Distro::Debian | Distro::Ubuntu | Distro::Mint => format!("apt_install {file_q}\n"),
                    };
                    let body = format!(
                        "# Pinned to the checksum from the release's signed manifest at export time.\nmkdir -p /var/cache/bootcraft/downloads\ncurl -fL -o {file_q} {url}\necho {sum} | sha256sum -c -\n{install}",
                        url = shell_quote(&resolved.url),
                        sum = shell_quote(&format!("{}  {file}", resolved.sha256)),
                    );
                    guarded(std::slice::from_ref(package), &body)
                }
                None => format!(
                    "echo {} >&2; exit 1\n",
                    shell_quote(&format!(
                        "Couldn't verify {package}'s signed release at export time; export again."
                    ))
                ),
            }
        }
        Step::SignedAppImage { package, manifest_url, public_key, install } => {
            match resolve(manifest_url, public_key, ReleaseFile::AppImage) {
                Some(resolved) => {
                    let file = format!("/var/cache/bootcraft/downloads/{}", resolved.file_name);
                    let file_q = shell_quote(&file);
                    let as_user = |from: &str, to: &Path, mode: &str| {
                        format!(
                            "runuser -u \"$user\" -- install -D -m {mode} {from} {}\n",
                            shell_quote(&to.to_string_lossy())
                        )
                    };
                    let body = format!(
                        "# Pinned to the checksum from the release's signed manifest at export time;\n# installed for you, so the app can update itself.\n[[ -n $user ]] || {{ echo 'Run this with sudo from your account.' >&2; exit 1; }}\nmkdir -p /var/cache/bootcraft/downloads\ncurl -fL -o {file_q} {url}\necho {sum} | sha256sum -c -\n{app}printf '%s' {desktop} | {entry}printf '%s' {icon_svg} | {icon}",
                        url = shell_quote(&resolved.url),
                        sum = shell_quote(&format!("{}  {file}", resolved.sha256)),
                        app = as_user(&file_q, &install.path, "755"),
                        desktop = shell_quote(&install.desktop),
                        entry = as_user("/dev/stdin", &install.desktop_entry, "644"),
                        icon_svg = shell_quote(&install.icon_svg),
                        icon = as_user("/dev/stdin", &install.icon, "644"),
                    );
                    format!(
                        "if [[ ! -f {} ]]; then\n  {}\nfi\n",
                        shell_quote(&install.path.to_string_lossy()),
                        body.trim_end().replace('\n', "\n  ")
                    )
                }
                None => format!(
                    "echo {} >&2; exit 1\n",
                    shell_quote(&format!(
                        "Couldn't verify {package}'s signed release at export time; export again."
                    ))
                ),
            }
        }
        Step::FlatpakRemote { name, url } => {
            format!(
                "flatpak remote-add --if-not-exists --system {} {}\n",
                shell_quote(name),
                shell_quote(url)
            )
        }
        Step::FlatpakInstall { remote, apps } => {
            format!("flatpak install -y --noninteractive --system {} {}\n", shell_quote(remote), quoted(apps))
        }
        Step::Edit { path, edit, if_exists } => edited(path, edit, *if_exists, root),
        Step::WriteFile { path, contents } => write_file(path, contents),
        Step::Download { url, path, sha256 } => {
            let path_q = shell_quote(&path.to_string_lossy());
            let dir = path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            format!(
                "mkdir -p {}\ncurl -fL -o {path_q} {}\necho {} | sha256sum -c -\n",
                shell_quote(&dir),
                shell_quote(url),
                shell_quote(&format!("{sha256}  {}", path.display()))
            )
        }
        Step::Swap { from, to } => format!(
            "if ! installed {to}; then\n  if installed {from}; then dnf -y swap {from} {to} --allowerasing\n  else dnf -y install --allowerasing {to}; fi\nfi\n",
            to = shell_quote(to),
            from = shell_quote(from)
        ),
        Step::AddToGroup { user, group } => {
            format!("usermod -aG {} {}\n", shell_quote(group), shell_quote(user))
        }
        Step::ForeignArch { arch } => format!("dpkg --add-architecture {}\napt_fresh=0\n", shell_quote(arch)),
        Step::RemoveSnaps => r#"# Snaps others depend on refuse to go until those are gone; retry each round.
while snaps="$(snap list 2>/dev/null | awk 'NR > 1 { print $1 }')" && [[ -n "$snaps" ]]; do
  removed=0
  for snap in $snaps; do snap remove --purge "$snap" && removed=1 || true; done
  ((removed)) || { echo "These snaps could not be removed: $snaps" >&2; exit 1; }
done
"#
        .to_owned(),
        Step::UbuntuDriver { open, max_branch } => {
            let filter = if *open {
                "grep -oE '^nvidia-driver-[0-9]+-open\\b'".to_owned()
            } else {
                let max = max_branch.unwrap_or(u32::MAX);
                format!("grep -oE '^nvidia-driver-[0-9]+(,|$)' | tr -d , | awk -F- '$3 <= {max}'")
            };
            format!(
                "driver=\"$(ubuntu-drivers list 2>/dev/null | {filter} | sort -V | tail -n1)\"\n[[ -n \"$driver\" ]] || {{ echo \"ubuntu-drivers lists no suitable NVIDIA driver\" >&2; exit 1; }}\napt_install \"$driver\"\n"
            )
        }
        Step::EnableService { unit } => format!("systemctl enable --now {}\n", shell_quote(unit)),
        Step::Run { argv, unless } => {
            let mut line = quoted(argv);
            // The script's own apt wrapper refreshes and remembers it.
            if argv.first().is_some_and(|a| a == "env") && argv.iter().any(|a| a == "apt-get") {
                let rest: Vec<String> = argv
                    .iter()
                    .skip_while(|a| *a != "apt-get")
                    .skip(1)
                    .filter(|a| !APT_GET.contains(&a.as_str()))
                    .cloned()
                    .collect();
                line = format!("apt {}", quoted(&rest));
                if rest.iter().any(|a| a == "update") {
                    line.push_str("; apt_fresh=1");
                }
            }
            guarded(unless, &format!("{line}\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlannedTask;

    fn plan_of(distro: Distro, steps: Vec<Step>) -> Plan {
        Plan {
            distro,
            tasks: vec![PlannedTask {
                id: "t".into(),
                title: "A task".into(),
                steps,
                reboot: false,
                requires: Vec::new(),
                required_by: None,
            }],
        }
    }

    fn no_resolve(_: &str, _: &str, _: ReleaseFile) -> Option<Resolved> {
        None
    }

    /// `bash -n`: the script parses.
    fn assert_parses(script: &str) {
        let mut child = std::process::Command::new("bash")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("bash");
        std::io::Write::write_all(child.stdin.as_mut().unwrap(), script.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{}\n{script}", String::from_utf8_lossy(&out.stderr));
    }

    #[test]
    fn every_step_kind_renders_valid_bash() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("etc")).unwrap();
        fs::write(root.path().join("etc/pacman.conf"), "[options]\n#Color\n").unwrap();
        let steps = vec![
            Step::install_unless(&["NVIDIA-MODULE"], &["nvidia-open-dkms", "linux-headers"]),
            Step::remove(&["kmines", "libreoffice*"]),
            Step::aur_unless(&["yay", "paru"], &["yay-bin"]),
            Step::SignedPackage {
                package: "comfypilot".into(),
                manifest_url: "https://x/m.json".into(),
                public_key: "k".into(),
            },
            Step::FlatpakRemote {
                name: "flathub".into(),
                url: "https://dl.flathub.org/repo/flathub.flatpakrepo".into(),
            },
            Step::FlatpakInstall { remote: "flathub".into(), apps: vec!["org.gimp.GIMP".into()] },
            Step::edit("/etc/pacman.conf", Edit::flag("options", "Color")),
            Step::edit_existing("/etc/lightdm/lightdm.conf", Edit::set_key("Seat:*", "greeter-session", "x")),
            Step::write_file("/etc/apt/preferences.d/no-snap.pref", "Package: snapd\nPin-Priority: -1\n"),
            Step::Download {
                url: "https://x/k.deb".into(),
                path: "/var/cache/bootcraft/downloads/k.deb".into(),
                sha256: "ab".repeat(32),
            },
            Step::Swap { from: "ffmpeg-free".into(), to: "ffmpeg".into() },
            Step::AddToGroup { user: "alice".into(), group: "sudo".into() },
            Step::ForeignArch { arch: "i386".into() },
            Step::RemoveSnaps,
            Step::UbuntuDriver { open: true, max_branch: None },
            Step::UbuntuDriver { open: false, max_branch: Some(580) },
            Step::EnableService { unit: "ufw.service".into() },
            Step::run_unless(&["ttf-mscorefonts-installer"], &["debconf-set-selections", "/tmp/x"]),
            Step::Run {
                argv: APT_GET.iter().chain(&["update"]).map(|a| (*a).to_owned()).collect(),
                unless: Vec::new(),
            },
        ];
        for distro in [Distro::Arch, Distro::Fedora, Distro::Debian, Distro::Ubuntu] {
            let script =
                to_script(&plan_of(distro, steps.clone()), root.path(), "2026-10-24", &mut no_resolve);
            assert_parses(&script);
        }
    }

    #[test]
    fn edits_are_written_as_finished_files() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("etc")).unwrap();
        fs::write(root.path().join("etc/pacman.conf"), "[options]\n#Color\n").unwrap();
        let steps = vec![Step::edit("/etc/pacman.conf", Edit::flag("options", "Color"))];
        let script = to_script(&plan_of(Distro::Arch, steps), root.path(), "today", &mut no_resolve);
        assert!(script.contains("backup /etc/pacman.conf\ncat > /etc/pacman.conf <<'BOOTCRAFT_EOF'\n[options]\nColor\nBOOTCRAFT_EOF\n"));
    }

    #[test]
    fn signed_releases_are_pinned_or_fail_loudly() {
        let step = Step::SignedPackage {
            package: "comfypilot".into(),
            manifest_url: "m".into(),
            public_key: "k".into(),
        };
        let root = tempfile::tempdir().unwrap();
        let mut resolve = |_: &str, _: &str, _: ReleaseFile| {
            Some(Resolved {
                url: "https://x/c.pkg.tar.zst".into(),
                sha256: "ff".repeat(32),
                file_name: "c.pkg.tar.zst".into(),
            })
        };
        let script =
            to_script(&plan_of(Distro::Arch, vec![step.clone()]), root.path(), "today", &mut resolve);
        assert!(
            script.contains(&format!("{}  /var/cache/bootcraft/downloads/c.pkg.tar.zst", "ff".repeat(32)))
        );
        let failing = to_script(&plan_of(Distro::Arch, vec![step]), root.path(), "today", &mut no_resolve);
        assert!(failing.contains("verify comfypilot") && failing.contains("exit 1"));
    }

    #[test]
    fn appimages_are_installed_as_the_user() {
        let step = Step::SignedAppImage {
            package: "comfypilot".into(),
            manifest_url: "m".into(),
            public_key: "k".into(),
            install: crate::task::AppImageInstall {
                path: "/home/alice/Applications/ComfyPilot.AppImage".into(),
                desktop_entry: "/home/alice/.local/share/applications/cp.desktop".into(),
                desktop: "[Desktop Entry]\nName=ComfyPilot's\nExec=\"/home/alice/Applications/ComfyPilot.AppImage\"\n"
                    .into(),
                icon: "/home/alice/.local/share/icons/hicolor/scalable/apps/cp.svg".into(),
                icon_svg: "<svg xmlns='http://www.w3.org/2000/svg'/>".into(),
            },
        };
        let root = tempfile::tempdir().unwrap();
        let mut resolve = |_: &str, _: &str, file: ReleaseFile| {
            assert_eq!(file, ReleaseFile::AppImage);
            Some(Resolved {
                url: "https://x/ComfyPilot-1.0.0-x86_64.AppImage".into(),
                sha256: "ff".repeat(32),
                file_name: "ComfyPilot-1.0.0-x86_64.AppImage".into(),
            })
        };
        let script = to_script(&plan_of(Distro::Ubuntu, vec![step]), root.path(), "today", &mut resolve);
        assert_parses(&script);
        assert!(script.contains(
            "runuser -u \"$user\" -- install -D -m 755 /var/cache/bootcraft/downloads/ComfyPilot-1.0.0-x86_64.AppImage /home/alice/Applications/ComfyPilot.AppImage"
        ), "{script}");
        assert!(script.contains("if [[ ! -f /home/alice/Applications/ComfyPilot.AppImage ]]; then"));
    }
}
