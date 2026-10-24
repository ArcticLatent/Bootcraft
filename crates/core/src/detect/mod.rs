//! Build a [`System`] from the running machine.
//!
//! Every file is read relative to a root directory so tests can point
//! detection at a fixture tree instead of `/`.

mod gpu;
mod os_release;

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::system::{Desktop, SecureBoot, SessionType, System};

pub use gpu::{lookup_pci_name, scan_gpus};
pub use os_release::{parse_os_release, read_os_release};

/// Desktop/session hints that come from the user's environment rather than
/// the filesystem. The helper runs as root without the user's environment, so
/// the GUI passes its own values along and [`Env::from_process`] falls back to
/// logind for the CLI.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub current_desktop: Option<String>,
    pub session_type: Option<String>,
    /// The person's account, also when running under sudo or pkexec.
    pub user: Option<String>,
}

impl Env {
    pub fn from_process() -> Self {
        let var = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
        let pkexec_user = var("PKEXEC_UID").and_then(|uid| user_name(Path::new("/"), &uid));
        let mut env = Self {
            current_desktop: var("XDG_CURRENT_DESKTOP").or_else(|| var("DESKTOP_SESSION")),
            session_type: var("XDG_SESSION_TYPE"),
            user: var("SUDO_USER").or(pkexec_user).or_else(|| var("USER")).filter(|u| u != "root"),
        };
        if env.current_desktop.is_none() {
            // Under sudo or pkexec the desktop variables are gone; ask logind
            // about the invoking user's graphical session instead.
            if let Some(uid) = var("PKEXEC_UID").or_else(|| var("SUDO_UID"))
                && let Some(session) = logind_display_session(&uid)
            {
                env.current_desktop = logind_session_property(&session, "Desktop");
                env.session_type = logind_session_property(&session, "Type");
            }
        }
        env
    }
}

/// The account name for `uid` in `<root>/etc/passwd`.
fn user_name(root: &Path, uid: &str) -> Option<String> {
    let passwd = fs::read_to_string(root.join("etc/passwd")).ok()?;
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() > 2 && fields[2] == uid).then(|| fields[0].to_owned())
    })
}

fn logind_display_session(uid: &str) -> Option<String> {
    run_value(&["loginctl", "show-user", uid, "-p", "Display", "--value"])
}

fn logind_session_property(session: &str, property: &str) -> Option<String> {
    run_value(&["loginctl", "show-session", session, "-p", property, "--value"])
}

fn run_value(argv: &[&str]) -> Option<String> {
    let output = Command::new(argv[0]).args(&argv[1..]).output().ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !value.is_empty()).then_some(value)
}

/// Detect the system under `root` (normally `/`).
pub fn detect(root: &Path, env: &Env) -> System {
    let gpus = scan_gpus(root);
    let nvidia = gpus.iter().filter_map(|g| g.nvidia_gen()).min();
    System {
        os: read_os_release(root),
        desktop: env.current_desktop.as_deref().map_or(Desktop::Unknown, Desktop::parse),
        session: env.session_type.as_deref().map_or(SessionType::Unknown, SessionType::parse),
        gpus,
        nvidia,
        secure_boot: secure_boot(root),
        kernel_release: read_trimmed(&root.join("proc/sys/kernel/osrelease")).unwrap_or_default(),
        arch_kernels: arch_kernels(root),
        ssh_server: ["usr/lib/systemd/system/sshd.service", "usr/lib/systemd/system/ssh.service"]
            .iter()
            .any(|unit| root.join(unit).exists()),
        memory_bytes: memory(root),
        disk_free_bytes: rustix::fs::statvfs(root).ok().map(|fs| fs.f_bavail.saturating_mul(fs.f_frsize)),
        user: env.user.clone(),
    }
}

/// `MemTotal` from /proc/meminfo, which is in kibibytes.
fn memory(root: &Path) -> Option<u64> {
    let meminfo = fs::read_to_string(root.join("proc/meminfo")).ok()?;
    let line = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().next()?.parse().ok()?;
    Some(kib * 1024)
}

fn secure_boot(root: &Path) -> SecureBoot {
    if !root.join("sys/firmware/efi").is_dir() {
        return SecureBoot::NotEfi;
    }
    // efivarfs prefixes the value with 4 bytes of attributes.
    let var = root.join("sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c");
    match fs::read(var) {
        Ok(bytes) if bytes.len() >= 5 => {
            if bytes[4] == 1 {
                SecureBoot::Enabled
            } else {
                SecureBoot::Disabled
            }
        }
        _ => SecureBoot::Unknown,
    }
}

fn arch_kernels(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join("usr/lib/modules")) else {
        return Vec::new();
    };
    let mut kernels: Vec<String> =
        entries.flatten().filter_map(|entry| read_trimmed(&entry.path().join("pkgbase"))).collect();
    kernels.sort();
    kernels.dedup();
    kernels
}

fn read_trimmed(path: &Path) -> Option<String> {
    let value = fs::read_to_string(path).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
