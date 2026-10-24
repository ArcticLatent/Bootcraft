//! Carry out a [`Plan`] as root.
//!
//! Each task is re-checked right before it runs, and each install step only
//! asks the package manager for packages that are neither installed nor
//! provided by something installed. A failed task skips the tasks that
//! require it; independent tasks still run.

pub mod aur;
mod runner;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use runner::{Cmd, Runner, SystemRunner, User};

use crate::edit::Edit;
use crate::plan::Plan;
use crate::probe::{Probe, Status};
use crate::protocol::{Event, Outcome, PlanEntry};
use crate::release;
use crate::system::{Distro, System};
use crate::task::Step;

pub struct Summary {
    pub ok: bool,
    pub reboot: bool,
}

pub struct Executor<R: Runner> {
    root: PathBuf,
    distro: Distro,
    kernel_release: String,
    runner: R,
    user: Option<User>,
    cancel: Arc<AtomicBool>,
}

/// Why a step stopped the task.
type StepError = String;

impl<R: Runner> Executor<R> {
    /// `root` is `/` except in tests; `user` builds AUR packages.
    pub fn new(root: &Path, sys: &System, runner: R, user: Option<User>) -> Option<Self> {
        Some(Self {
            root: root.to_owned(),
            distro: sys.distro()?,
            kernel_release: sys.kernel_release.clone(),
            runner,
            user,
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Set to stop after the step that is running.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    pub fn execute(&mut self, plan: &Plan, emit: &mut dyn FnMut(Event)) -> Summary {
        emit(Event::Plan {
            tasks: plan
                .tasks
                .iter()
                .map(|t| PlanEntry {
                    id: t.id.clone(),
                    title: t.title.clone(),
                    required_by: t.required_by.clone(),
                })
                .collect(),
        });

        let mut failed: HashSet<&str> = HashSet::new();
        let mut reboot = false;
        for task in &plan.tasks {
            let finish = |outcome, message: Option<String>| Event::TaskFinished {
                id: task.id.clone(),
                outcome,
                message,
            };
            if self.cancel.load(Ordering::SeqCst) {
                emit(finish(Outcome::Cancelled, None));
                continue;
            }
            if let Some(dep) = task.requires.iter().find(|r| failed.contains(r.as_str())) {
                failed.insert(&task.id);
                emit(finish(Outcome::Skipped, Some(format!("requires {dep}, which failed"))));
                continue;
            }
            if self.probe().status(&task.steps) == Status::Done {
                emit(finish(Outcome::AlreadyDone, None));
                continue;
            }

            emit(Event::TaskStarted { id: task.id.clone() });
            let mut result = Ok(());
            for step in &task.steps {
                if self.cancel.load(Ordering::SeqCst) {
                    result = Err(None);
                    break;
                }
                emit(Event::Step { id: task.id.clone(), description: step.describe(self.distro) });
                if let Err(message) = self.step(step, emit) {
                    result = Err(Some(message));
                    break;
                }
            }
            match result {
                Ok(()) => {
                    reboot |= task.reboot;
                    emit(finish(Outcome::Done, None));
                }
                Err(None) => emit(finish(Outcome::Cancelled, None)),
                Err(Some(message)) => {
                    failed.insert(&task.id);
                    emit(finish(Outcome::Failed, Some(message)));
                }
            }
        }
        Summary {
            ok: failed.is_empty() && !self.cancel.load(Ordering::SeqCst),
            reboot: reboot || self.kernel_replaced(),
        }
    }

    /// Package state changes as steps run, so read it fresh each time.
    fn probe(&self) -> Probe {
        Probe::new(&self.root, self.distro)
    }

    /// An upgrade removed the running kernel's modules, so new modules
    /// (including freshly built drivers) only load after a reboot.
    fn kernel_replaced(&self) -> bool {
        self.distro == Distro::Arch
            && !self.kernel_release.is_empty()
            && !self.root.join("usr/lib/modules").join(&self.kernel_release).is_dir()
    }

    fn step(&mut self, step: &Step, emit: &mut dyn FnMut(Event)) -> Result<(), StepError> {
        let probe = self.probe();
        let note = |emit: &mut dyn FnMut(Event), line: &str| emit(Event::Output { line: line.to_owned() });
        match step {
            Step::Install { packages, unless } | Step::AurInstall { packages, unless } => {
                if let Some(provide) = unless.iter().find(|p| probe.is_installed(p) == Some(true)) {
                    note(emit, &format!("{provide} is already installed; skipping."));
                    return Ok(());
                }
                let missing: Vec<String> =
                    packages.iter().filter(|p| probe.is_installed(p) != Some(true)).cloned().collect();
                if missing.is_empty() {
                    note(emit, "Everything is already installed.");
                    return Ok(());
                }
                if matches!(step, Step::AurInstall { .. }) {
                    self.aur_install(&probe, &missing, emit)
                } else {
                    self.install(&missing, emit)
                }
            }
            Step::Remove { packages } => {
                let mut installed = Vec::new();
                for pattern in packages {
                    installed.extend(probe.installed_matching(pattern).unwrap_or_default());
                }
                if installed.is_empty() {
                    note(emit, "Nothing to remove.");
                    return Ok(());
                }
                self.remove(&installed, emit)
            }
            Step::SignedPackage { package, manifest_url, public_key } => {
                if probe.is_installed(package) == Some(true) {
                    note(emit, &format!("{package} is already installed."));
                    return Ok(());
                }
                self.signed_package(manifest_url, public_key, emit)
            }
            Step::FlatpakRemote { name, url } => self.run(
                Cmd::new([
                    "flatpak",
                    "remote-add",
                    "--if-not-exists",
                    "--system",
                    name.as_str(),
                    url.as_str(),
                ]),
                emit,
            ),
            Step::FlatpakInstall { remote, apps } => {
                let mut argv =
                    vec!["flatpak", "install", "-y", "--noninteractive", "--system", remote.as_str()];
                argv.extend(apps.iter().map(String::as_str));
                self.run(Cmd::new(argv), emit)
            }
            Step::Edit { path, edit, if_exists } => {
                let target = self.root.join(path.strip_prefix("/").unwrap_or(path));
                if *if_exists && !target.exists() {
                    note(emit, &format!("{} doesn't exist; nothing to change.", path.display()));
                    return Ok(());
                }
                self.edit(path, edit, emit)
            }
            Step::EnableService { unit } => {
                self.run(Cmd::new(["systemctl", "enable", "--now", unit.as_str()]), emit)
            }
            Step::Run { argv } => {
                if self.distro == Distro::Arch && argv.first().is_some_and(|a| a == "pacman") {
                    self.check_pacman_lock()?;
                }
                self.run(Cmd::new(argv.clone()), emit)
            }
        }
    }

    fn run(&mut self, cmd: Cmd, emit: &mut dyn FnMut(Event)) -> Result<(), StepError> {
        emit(Event::Output { line: cmd.display() });
        let program = cmd.argv[0].clone();
        // pacman, makepkg and friends end with an `error:` line that says what went wrong.
        let mut last_error = None;
        let result = self.runner.run(&cmd, &mut |line| {
            if let Some(error) = line.strip_prefix("error: ") {
                last_error = Some(error.trim().to_owned());
            }
            emit(Event::Output { line: line.to_owned() });
        });
        match result {
            Ok(true) => Ok(()),
            Ok(false) => Err(match last_error {
                Some(error) => format!("{program} failed: {error}"),
                None => format!("{program} failed; see the log for details"),
            }),
            Err(err) => Err(format!("could not start {program}: {err}")),
        }
    }

    fn check_pacman_lock(&self) -> Result<(), StepError> {
        if self.root.join("var/lib/pacman/db.lck").exists() {
            return Err("another package manager is running (/var/lib/pacman/db.lck exists); \
                        close it and try again"
                .into());
        }
        Ok(())
    }

    fn pacman(
        &mut self,
        args: &[&str],
        packages: &[String],
        emit: &mut dyn FnMut(Event),
    ) -> Result<(), StepError> {
        self.check_pacman_lock()?;
        let argv =
            std::iter::once("pacman").chain(args.iter().copied()).chain(packages.iter().map(String::as_str));
        self.run(Cmd::new(argv), emit)
    }

    /// `pacman -U` for package files. With `--noconfirm`, pacman refuses to
    /// remove a conflicting package, which blocks installing the successor of
    /// a renamed app (ComfyPilot replaces arctic-comfyui-helper). When every
    /// installed conflict is one the new package declares it `replaces`,
    /// approve exactly that question (`--ask=4`, ALPM_QUESTION_CONFLICT_PKG);
    /// any other conflict stops with an explanation instead.
    fn pacman_upgrade(
        &mut self,
        files: &[String],
        as_deps: bool,
        emit: &mut dyn FnMut(Event),
    ) -> Result<(), StepError> {
        let probe = self.probe();
        let mut replaced = Vec::new();
        for file in files {
            let query = Cmd::new(["env", "LC_ALL=C", "pacman", "-Qip", file.as_str()]);
            let info = match self.runner.capture(&query) {
                Ok((true, info)) => info,
                _ => return Err(format!("could not read the package metadata of {file}")),
            };
            let (conflicts, replaces) =
                (package_field(&info, "Conflicts With"), package_field(&info, "Replaces"));
            for conflict in &conflicts {
                for installed in probe.installed_matching(conflict).unwrap_or_default() {
                    if replaces.contains(conflict) {
                        replaced.push(installed);
                    } else {
                        return Err(format!(
                            "{} conflicts with the installed {installed}; remove {installed} first to switch",
                            file.rsplit('/').next().unwrap_or(file)
                        ));
                    }
                }
            }
        }
        let mut args = vec!["-U", "--needed", "--noconfirm"];
        if as_deps {
            args.push("--asdeps");
        }
        if !replaced.is_empty() {
            emit(Event::Output {
                line: format!("Replacing {}, as the new package declares.", replaced.join(", ")),
            });
            args.push("--ask=4");
        }
        self.pacman(&args, files, emit)
    }

    fn install(&mut self, packages: &[String], emit: &mut dyn FnMut(Event)) -> Result<(), StepError> {
        match self.distro {
            Distro::Arch => self.pacman(&["-S", "--needed", "--noconfirm"], packages, emit),
            other => Err(format!("installing packages on {} is not implemented yet", other.name())),
        }
    }

    fn remove(&mut self, packages: &[String], emit: &mut dyn FnMut(Event)) -> Result<(), StepError> {
        match self.distro {
            Distro::Arch => self.pacman(&["-Rns", "--noconfirm"], packages, emit),
            other => Err(format!("removing packages on {} is not implemented yet", other.name())),
        }
    }

    fn aur_install(
        &mut self,
        probe: &Probe,
        targets: &[String],
        emit: &mut dyn FnMut(Event),
    ) -> Result<(), StepError> {
        let user = self.user.clone().ok_or(
            "AUR packages are built as a regular user; start Bootcraft from your own account \
             (with sudo or from the app), not as root",
        )?;
        let plan = {
            let mut resolver = ArchResolver { runner: &mut self.runner, probe };
            aur::resolve(&mut resolver, targets)?
        };
        if !plan.repo_deps.is_empty() {
            self.pacman(&["-S", "--needed", "--noconfirm", "--asdeps"], &plan.repo_deps, emit)?;
        }
        let cache = user.home.join(".cache/bootcraft/aur");
        for build in &plan.builds {
            let dir = cache.join(&build.base);
            let dir_str = dir.to_string_lossy().into_owned();
            let as_user = |argv: &[&str]| Cmd::new(argv.iter().copied()).as_user(&user);
            emit(Event::Output { line: format!("Building {} from the AUR", build.base) });

            self.run(as_user(&["mkdir", "-p", &cache.to_string_lossy()]), emit)?;
            if dir.join(".git").is_dir() {
                self.run(as_user(&["git", "-C", &dir_str, "fetch", "--depth", "1", "origin"]), emit)?;
                self.run(as_user(&["git", "-C", &dir_str, "reset", "--hard", "FETCH_HEAD"]), emit)?;
                self.run(as_user(&["git", "-C", &dir_str, "clean", "-fdx"]), emit)?;
            } else {
                let url = format!("https://aur.archlinux.org/{}.git", build.base);
                self.run(as_user(&["git", "clone", "--depth", "1", &url, &dir_str]), emit)?;
            }
            self.run(as_user(&["makepkg", "--noconfirm", "--nocheck", "-f"]).in_dir(&dir), emit)?;

            let (ok, list) = self
                .runner
                .capture(&as_user(&["makepkg", "--packagelist"]).in_dir(&dir))
                .map_err(|e| format!("could not list built packages: {e}"))?;
            if !ok {
                return Err(format!("could not list the packages built from {}", build.base));
            }
            let files = |wanted: &[String]| -> Result<Vec<String>, StepError> {
                wanted
                    .iter()
                    .map(|name| {
                        list.lines()
                            .find(|path| package_file_name(path) == Some(name.as_str()))
                            .map(str::to_owned)
                            .ok_or_else(|| format!("makepkg did not produce {name}"))
                    })
                    .collect()
            };
            let (deps, explicit) = (files(&build.deps)?, files(&build.explicit)?);
            if !deps.is_empty() {
                self.pacman_upgrade(&deps, true, emit)?;
            }
            if !explicit.is_empty() {
                self.pacman_upgrade(&explicit, false, emit)?;
            }
        }
        Ok(())
    }

    /// Fetch and verify the manifest, download this distribution's package,
    /// check its SHA-256, then install it.
    fn signed_package(
        &mut self,
        manifest_url: &str,
        public_key: &str,
        emit: &mut dyn FnMut(Event),
    ) -> Result<(), StepError> {
        let fetch = Cmd::new(["curl", "-fsSL", "--max-time", "60", manifest_url]);
        emit(Event::Output { line: fetch.display() });
        let manifest = match self.runner.capture(&fetch) {
            Ok((true, body)) => release::verify(&body, public_key)?,
            Ok((false, _)) => return Err(format!("could not download {manifest_url}")),
            Err(err) => return Err(format!("could not run curl: {err}")),
        };
        emit(Event::Output {
            line: format!("Release {} of {}: signature verified.", manifest.version, manifest.repository),
        });
        let asset = release::asset_for(&manifest, self.distro)
            .ok_or_else(|| {
                format!("release {} has no x86-64 package for {}", manifest.version, self.distro.name())
            })?
            .clone();

        let cache = self.root.join("var/cache/bootcraft/downloads");
        fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
        let file = cache.join(&asset.name);
        let file_str = file.to_string_lossy().into_owned();
        self.run(
            Cmd::new(["curl", "-fL", "--max-time", "1800", "-o", &file_str, &asset.download_url]),
            emit,
        )?;
        let digest = release::sha256_file(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        if !digest.eq_ignore_ascii_case(&asset.sha256) {
            let _ = fs::remove_file(&file);
            return Err(format!("{} does not match the signed checksum; it was deleted", asset.name));
        }
        emit(Event::Output { line: format!("{}: checksum verified.", asset.name) });
        match self.distro {
            Distro::Arch => self.pacman_upgrade(&[file_str], false, emit),
            other => Err(format!("installing packages on {} is not implemented yet", other.name())),
        }
    }

    fn edit(&mut self, path: &Path, edit: &Edit, emit: &mut dyn FnMut(Event)) -> Result<(), StepError> {
        let target = self.root.join(path.strip_prefix("/").unwrap_or(path));
        let fail = |e: io::Error| format!("{}: {e}", path.display());
        let original = match fs::read_to_string(&target) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(fail(e)),
        };
        let text = original.as_deref().unwrap_or_default();
        if original.is_some() && edit.is_applied(text) {
            emit(Event::Output { line: format!("{} already has this change.", path.display()) });
            return Ok(());
        }
        let updated = edit.apply(text);
        let mode = fs::metadata(&target).map_or(0o644, |m| m.permissions().mode() & 0o7777);
        if let Some(original) = &original {
            // Keep the first original around; later runs must not overwrite it.
            let backup = PathBuf::from(format!("{}.bootcraft.bak", target.display()));
            if !backup.exists() {
                fs::write(&backup, original).map_err(fail)?;
            }
        } else if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(fail)?;
        }
        // Write a sibling and rename, so the file is never half-written.
        let file_name = target.file_name().unwrap_or_default().to_string_lossy();
        let temp = target.with_file_name(format!(".{file_name}.bootcraft.tmp"));
        fs::write(&temp, updated).map_err(fail)?;
        fs::set_permissions(&temp, fs::Permissions::from_mode(mode)).map_err(fail)?;
        fs::rename(&temp, &target).map_err(fail)?;
        emit(Event::Output { line: format!("Updated {}: {edit}", path.display()) });
        Ok(())
    }
}

/// A list field from `LC_ALL=C pacman -Qi(p)`, e.g. `Replaces : a b`, without
/// version constraints. `None` means an empty list.
fn package_field(info: &str, name: &str) -> Vec<String> {
    info.lines()
        .find_map(|line| line.strip_prefix(name)?.trim_start().strip_prefix(':'))
        .map(|value| {
            value
                .split_whitespace()
                .filter(|v| *v != "None")
                .map(|v| crate::probe::strip_version(v).to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// `/x/nvidia-580xx-dkms-580.196.07-1-x86_64.pkg.tar.zst` -> `nvidia-580xx-dkms`
/// (pkgname-pkgver-pkgrel-arch.pkg.tar.*).
fn package_file_name(path: &str) -> Option<&str> {
    let file = path.rsplit('/').next()?;
    let stem = &file[..file.find(".pkg.tar")?];
    let mut parts = stem.rsplitn(4, '-');
    let (_arch, _rel, _ver) = (parts.next()?, parts.next()?, parts.next()?);
    parts.next()
}

struct ArchResolver<'a, R: Runner> {
    runner: &'a mut R,
    probe: &'a Probe,
}

impl<R: Runner> aur::Resolver for ArchResolver<'_, R> {
    fn installed(&self, dep: &str) -> bool {
        self.probe.is_installed(dep) == Some(true)
    }

    fn in_repos(&mut self, dep: &str) -> bool {
        let query = Cmd::new(["pacman", "-Sp", "--print-format", "%n", dep]);
        self.runner.capture(&query).is_ok_and(|(ok, _)| ok)
    }

    fn aur_info(&mut self, names: &[String]) -> Result<Vec<aur::AurInfo>, String> {
        let query = Cmd::new(["curl", "-fsSLg", "--max-time", "30", &aur::rpc_url(names)]);
        match self.runner.capture(&query) {
            Ok((true, body)) => aur::parse_rpc(&body),
            Ok((false, _)) => Err("could not reach the AUR (aur.archlinux.org)".into()),
            Err(err) => Err(format!("could not run curl: {err}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{package_field, package_file_name};

    #[test]
    fn package_fields() {
        let info = "Name            : comfypilot\nConflicts With  : arctic-comfyui-helper old>=1\nReplaces        : None\n";
        assert_eq!(package_field(info, "Conflicts With"), ["arctic-comfyui-helper", "old"]);
        assert!(package_field(info, "Replaces").is_empty());
        assert!(package_field(info, "Provides").is_empty());
    }

    #[test]
    fn package_file_names() {
        assert_eq!(
            package_file_name("/home/u/.cache/x/nvidia-580xx-dkms-580.196.07-1-x86_64.pkg.tar.zst"),
            Some("nvidia-580xx-dkms")
        );
        assert_eq!(package_file_name("paru-bin-2.1.0-1-x86_64.pkg.tar.xz"), Some("paru-bin"));
        assert_eq!(package_file_name("mint-artwork-2:1.9.5-0-any.pkg.tar.zst"), Some("mint-artwork"));
        assert_eq!(package_file_name("README"), None);
    }
}
