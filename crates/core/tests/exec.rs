mod common;

use std::cell::RefCell;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use bootcraft_core::catalog::catalog;
use bootcraft_core::exec::{Cmd, Executor, Runner, Summary, User};
use bootcraft_core::protocol::{Event, Outcome};
use bootcraft_core::{Selection, System, plan};
use common::{ARCH, Fixture, PACMAN_CONF};

type Respond = Box<dyn FnMut(&[String]) -> (bool, String)>;

/// Records commands instead of running them; `respond` decides the result.
struct FakeRunner {
    log: Rc<RefCell<Vec<Cmd>>>,
    respond: Respond,
}

impl Runner for FakeRunner {
    fn run(&mut self, cmd: &Cmd, output: &mut dyn FnMut(&str)) -> io::Result<bool> {
        self.log.borrow_mut().push(cmd.clone());
        let (ok, out) = (self.respond)(&cmd.argv);
        out.lines().for_each(output);
        Ok(ok)
    }

    fn capture(&mut self, cmd: &Cmd) -> io::Result<(bool, String)> {
        self.log.borrow_mut().push(cmd.clone());
        Ok((self.respond)(&cmd.argv))
    }
}

struct Run {
    summary: Summary,
    events: Vec<Event>,
    commands: Vec<String>,
}

impl Run {
    fn outcome(&self, id: &str) -> (Outcome, Option<String>) {
        self.events
            .iter()
            .find_map(|e| match e {
                Event::TaskFinished { id: done, outcome, message } if done == id => {
                    Some((*outcome, message.clone()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no result for {id}"))
    }
}

fn execute(f: &Fixture, sys: &System, select: &[&str], user: Option<User>, respond: Respond) -> Run {
    let selection =
        Selection { tasks: select.iter().map(|s| (*s).to_owned()).collect(), ..Selection::default() };
    let plan = plan(sys, &catalog(sys), &selection).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runner = FakeRunner { log: Rc::clone(&log), respond };
    let mut executor = Executor::new(f.root(), sys, runner, user).unwrap();
    let mut events = Vec::new();
    let summary = executor.execute(&plan, &mut |e| events.push(e));
    let commands = log.borrow().iter().map(Cmd::display).collect();
    Run { summary, events, commands }
}

fn succeed() -> Respond {
    Box::new(|_| (true, String::new()))
}

/// Stock Arch box with an RTX 4070, multilib already on, pacman installed.
fn arch_box() -> Fixture {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, 0x2786, true)
        .kernel("7.2.11-arch1-1", "linux")
        .file("etc/pacman.conf", &PACMAN_CONF.replace("#[multilib]\n#Include", "[multilib]\nInclude"))
        .package("pacman", &["libalpm.so=16-64"]);
    f
}

#[test]
fn installs_only_what_is_missing() {
    let f = arch_box();
    f.package("nvidia-open-dkms-tkg", &["nvidia-open", "NVIDIA-MODULE"])
        .package("nvidia-utils-tkg", &["nvidia-utils=615.78.08"]);
    let sys = f.detect("KDE");
    let run = execute(&f, &sys, &["gpu.nvidia"], None, succeed());

    assert_eq!(
        run.commands,
        ["$ pacman -Syu --noconfirm", "$ pacman -S --needed --noconfirm lib32-nvidia-utils"]
    );
    assert_eq!(run.outcome("arch.multilib").0, Outcome::AlreadyDone);
    assert_eq!(run.outcome("gpu.nvidia").0, Outcome::Done);
    assert!(run.summary.ok);
    assert!(run.summary.reboot, "a driver install needs a reboot");
}

#[test]
fn failure_skips_dependents_but_not_independent_tasks() {
    let f = arch_box();
    let sys = f.detect("KDE");
    let respond: Respond = Box::new(|argv| (argv.get(1).map(String::as_str) != Some("-Syu"), String::new()));
    let run = execute(&f, &sys, &["arch.pacman-tweaks", "media.codecs", "debloat.kde"], None, respond);

    let (outcome, message) = run.outcome("system.update");
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(message.as_deref(), Some("pacman failed; see the log for details"));
    let (outcome, message) = run.outcome("media.codecs");
    assert_eq!(outcome, Outcome::Skipped);
    assert_eq!(message.as_deref(), Some("requires system.update, which failed"));
    assert_eq!(run.outcome("arch.pacman-tweaks").0, Outcome::Done);
    // Nothing from the KDE list is installed.
    assert_eq!(run.outcome("debloat.kde").0, Outcome::AlreadyDone);
    assert!(!run.summary.ok);
}

#[test]
fn removal_expands_prefixes_to_real_package_names() {
    let f = arch_box();
    f.package("libreoffice-fresh", &[]).package("libreoffice-fresh-de", &[]).package("gvim", &["vim"]);
    let sys = f.detect("GNOME");
    let run = execute(&f, &sys, &["debloat.libreoffice", "debloat.gnome"], None, succeed());
    assert_eq!(
        run.commands,
        ["$ pacman -Rns --noconfirm libreoffice-fresh libreoffice-fresh-de"],
        "gvim only provides vim and must not be removed as `vim`"
    );
}

#[test]
fn config_edits_are_atomic_backed_up_and_idempotent() {
    let f = arch_box();
    let conf = f.root().join("etc/pacman.conf");
    fs::set_permissions(&conf, fs::Permissions::from_mode(0o640)).unwrap();
    let original = fs::read_to_string(&conf).unwrap();
    let sys = f.detect("KDE");

    let run = execute(&f, &sys, &["arch.pacman-tweaks"], None, succeed());
    assert_eq!(run.outcome("arch.pacman-tweaks").0, Outcome::Done);
    assert!(run.commands.is_empty());
    let updated = fs::read_to_string(&conf).unwrap();
    assert!(updated.contains("\nColor\n") && updated.contains("\nILoveCandy\n"));
    assert_eq!(fs::metadata(&conf).unwrap().permissions().mode() & 0o777, 0o640);
    let backup = f.root().join("etc/pacman.conf.bootcraft.bak");
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);

    let again = execute(&f, &sys, &["arch.pacman-tweaks"], None, succeed());
    assert_eq!(again.outcome("arch.pacman-tweaks").0, Outcome::AlreadyDone);
    assert_eq!(fs::read_to_string(&backup).unwrap(), original, "the first backup is kept");
}

#[test]
fn a_held_pacman_lock_fails_without_running_pacman() {
    let f = arch_box();
    f.file("var/lib/pacman/db.lck", "");
    let sys = f.detect("KDE");
    let run = execute(&f, &sys, &["system.update"], None, succeed());
    assert!(run.commands.is_empty());
    let (outcome, message) = run.outcome("system.update");
    assert_eq!(outcome, Outcome::Failed);
    assert!(message.unwrap().contains("another package manager is running"));
}

#[test]
fn aur_packages_are_built_as_the_user_and_installed_as_root() {
    let f = arch_box();
    f.package("base-devel", &[]).package("git", &[]);
    let sys = f.detect("KDE");
    let home = f.root().join("home/burce");
    let user = User { name: "burce".into(), uid: 1000, home: home.clone() };
    let built = home.join(".cache/bootcraft/aur/yay-bin/yay-bin-13.0.1-1-x86_64.pkg.tar.zst");
    let built_str = built.display().to_string();
    let listed = built_str.clone();
    let respond: Respond = Box::new(move |argv| {
        match argv[0].as_str() {
        "curl" => (
            true,
            r#"{"type":"multiinfo","results":[{"Name":"yay-bin","PackageBase":"yay-bin","Depends":["pacman>6.1","git"]}]}"#.into(),
        ),
        "makepkg" if argv.get(1).map(String::as_str) == Some("--packagelist") => {
            (true, format!("{listed}\n{listed}-debug-ignored\n"))
        }
        _ => (true, String::new()),
    }
    });
    let run = execute(&f, &sys, &["arch.aur-helper"], Some(user), respond);
    assert_eq!(run.outcome("arch.aur-helper").0, Outcome::Done, "{:#?}", run.events);

    let cache = home.join(".cache/bootcraft/aur");
    let expected = [
        "$ pacman -Syu --noconfirm".to_owned(),
        "$ curl -fsSLg --max-time 30 https://aur.archlinux.org/rpc/v5/info?arg[]=yay-bin".to_owned(),
        format!("$ mkdir -p {}  (as burce)", cache.display()),
        format!(
            "$ git clone --depth 1 https://aur.archlinux.org/yay-bin.git {}  (as burce)",
            cache.join("yay-bin").display()
        ),
        "$ makepkg --noconfirm --nocheck -f  (as burce)".to_owned(),
        "$ makepkg --packagelist  (as burce)".to_owned(),
        format!("$ env LC_ALL=C pacman -Qip {built_str}"),
        format!("$ pacman -U --needed --noconfirm {built_str}"),
    ];
    assert_eq!(run.commands, expected);
    // git and libalpm.so are already satisfied, so no repository deps were queried.
    assert!(!run.commands.iter().any(|c| c.contains("pacman -Sp")));
}

#[test]
fn aur_without_a_regular_user_explains_why() {
    let f = arch_box();
    f.package("base-devel", &[]).package("git", &[]);
    let sys = f.detect("KDE");
    let run = execute(&f, &sys, &["arch.aur-helper"], None, succeed());
    let (outcome, message) = run.outcome("arch.aur-helper");
    assert_eq!(outcome, Outcome::Failed);
    assert!(message.unwrap().contains("built as a regular user"));
}

#[test]
fn cancel_stops_before_anything_runs() {
    let f = arch_box();
    let sys = f.detect("KDE");
    let selection = Selection { tasks: vec!["system.update".into()], ..Selection::default() };
    let plan = plan(&sys, &catalog(&sys), &selection).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let runner = FakeRunner { log: Rc::clone(&log), respond: succeed() };
    let mut executor = Executor::new(f.root(), &sys, runner, None).unwrap();
    executor.cancel_flag().store(true, Ordering::SeqCst);
    let mut events = Vec::new();
    let summary = executor.execute(&plan, &mut |e| events.push(e));
    assert!(log.borrow().is_empty());
    assert!(!summary.ok);
    assert!(events.iter().any(|e| matches!(e, Event::TaskFinished { outcome: Outcome::Cancelled, .. })));
}

#[test]
fn reboot_when_the_running_kernel_was_replaced() {
    let f = arch_box();
    let sys = f.detect("KDE");
    let run = execute(&f, &sys, &["system.update"], None, succeed());
    assert!(!run.summary.reboot);

    // pacman -Syu removed the modules of the running kernel.
    fs::remove_dir_all(f.root().join("usr/lib/modules/7.2.11-arch1-1")).unwrap();
    let run = execute(&f, &sys, &["system.update"], None, succeed());
    assert!(run.summary.reboot);
}

mod signed {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use bootcraft_core::Plan;
    use bootcraft_core::Step;
    use bootcraft_core::plan::PlannedTask;
    use bootcraft_core::release::sha256_file;
    use bootcraft_core::system::Distro;
    use ed25519_dalek::{Signer, SigningKey};

    /// A release manifest in ComfyPilot's format, signed with a test key.
    fn signed_manifest(asset: &str, sha256: &str) -> (String, String) {
        let key = SigningKey::from_bytes(&[42; 32]);
        let url = format!("https://example.test/{asset}");
        let mut payload = b"arctic-linux-release-manifest\x00v1\x00".to_vec();
        for field in ["2.0.0", "v2.0.0", "ArcticLatent/Test", asset, sha256, &url] {
            payload.extend_from_slice(field.as_bytes());
            payload.push(0);
        }
        let signature = STANDARD.encode(key.sign(&payload).to_bytes());
        let json = serde_json::json!({
            "version": "2.0.0", "tag": "v2.0.0", "repository": "ArcticLatent/Test",
            "assets": [{"name": asset, "sha256": sha256, "download_url": url}],
            "signature": signature,
        });
        (json.to_string(), STANDARD.encode(key.verifying_key().to_bytes()))
    }

    /// `pacman -Qip` output for a package without conflicts.
    const NO_CONFLICTS: &str = "Name            : testpkg\nConflicts With  : None\nReplaces        : None\n";

    fn run(f: &Fixture, sys: &System, download: &'static [u8], package_info: &'static str) -> Run {
        // The manifest promises the checksum of the genuine package.
        let genuine = f.root().join("genuine");
        fs::write(&genuine, b"genuine package").unwrap();
        let sha = sha256_file(&genuine).unwrap();
        let (manifest, key) = signed_manifest("testpkg-2.0.0-1-x86_64.pkg.tar.zst", &sha);
        let plan = Plan {
            distro: Distro::Arch,
            tasks: vec![PlannedTask {
                id: "app.testpkg".into(),
                title: "Test package".into(),
                steps: vec![Step::SignedPackage {
                    package: "testpkg".into(),
                    manifest_url: "https://example.test/linux-release.json".into(),
                    public_key: key,
                }],
                reboot: false,
                requires: Vec::new(),
                required_by: None,
            }],
        };
        let respond: Respond = Box::new(move |argv| match argv {
            [curl, flag, ..] if curl == "curl" && flag == "-fsSL" => (true, manifest.clone()),
            [env, _, pacman, query, _] if env == "env" && pacman == "pacman" && query == "-Qip" => {
                (true, package_info.to_owned())
            }
            [curl, .., out_flag, path, _url] if curl == "curl" && out_flag == "-o" => {
                fs::write(path, download).unwrap();
                (true, String::new())
            }
            _ => (true, String::new()),
        });
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut executor =
            Executor::new(f.root(), sys, FakeRunner { log: Rc::clone(&log), respond }, None).unwrap();
        let mut events = Vec::new();
        let summary = executor.execute(&plan, &mut |e| events.push(e));
        let commands = log.borrow().iter().map(Cmd::display).collect();
        Run { summary, events, commands }
    }

    #[test]
    fn verified_download_is_installed() {
        let f = arch_box();
        let sys = f.detect("KDE");
        let run = run(&f, &sys, b"genuine package", NO_CONFLICTS);
        assert_eq!(run.outcome("app.testpkg").0, Outcome::Done, "{:#?}", run.events);
        let file = f.root().join("var/cache/bootcraft/downloads/testpkg-2.0.0-1-x86_64.pkg.tar.zst");
        assert_eq!(
            run.commands.last().unwrap(),
            &format!("$ pacman -U --needed --noconfirm {}", file.display())
        );
    }

    /// A fake system for one release: the manifest, the download, `pacman -Qip`.
    fn release_responder(manifest: String, download: &'static [u8]) -> Respond {
        Box::new(move |argv| match argv {
            [curl, flag, ..] if curl == "curl" && flag == "-fsSL" => (true, manifest.clone()),
            [env, _, pacman, query, _] if env == "env" && pacman == "pacman" && query == "-Qip" => {
                (true, NO_CONFLICTS.to_owned())
            }
            [curl, .., out_flag, path, _url] if curl == "curl" && out_flag == "-o" => {
                fs::write(path, download).unwrap();
                (true, String::new())
            }
            _ => (true, String::new()),
        })
    }

    fn genuine_sha(f: &Fixture) -> String {
        let genuine = f.root().join("genuine");
        fs::write(&genuine, b"genuine package").unwrap();
        sha256_file(&genuine).unwrap()
    }

    #[test]
    fn self_update_installs_only_a_newer_release() {
        let f = arch_box();
        let sys = f.detect("KDE");
        let (manifest, key) = signed_manifest("testpkg-2.0.0-1-x86_64.pkg.tar.zst", &genuine_sha(&f));
        let update = |current: &str| {
            let log = Rc::new(RefCell::new(Vec::new()));
            let runner = FakeRunner {
                log: Rc::clone(&log),
                respond: release_responder(manifest.clone(), b"genuine package"),
            };
            let mut executor = Executor::new(f.root(), &sys, runner, None).unwrap();
            let result = executor.update_from_release(
                "https://example.test/linux-release.json",
                &key,
                current,
                &mut |_| {},
            );
            let commands: Vec<String> = log.borrow().iter().map(Cmd::display).collect();
            (result, commands)
        };

        let (result, commands) = update("1.4.0");
        assert_eq!(result, Ok("2.0.0".to_owned()));
        assert!(commands.last().unwrap().starts_with("$ pacman -U --needed --noconfirm "), "{commands:?}");

        let (result, commands) = update("2.0.0");
        assert!(result.unwrap_err().contains("not newer"));
        assert!(!commands.iter().any(|c| c.contains("-o ")), "nothing downloaded: {commands:?}");
    }

    #[test]
    fn appimage_is_installed_for_the_user_with_a_menu_entry() {
        let f = arch_box();
        let sys = f.detect("KDE");
        let (manifest, key) = signed_manifest("Test-2.0.0-x86_64.AppImage", &genuine_sha(&f));
        let home = std::path::PathBuf::from("/home/burce");
        let install = bootcraft_core::task::AppImageInstall {
            path: home.join("Applications/Test.AppImage"),
            desktop_entry: home.join(".local/share/applications/test.desktop"),
            desktop: "[Desktop Entry]\nName=Test\n".into(),
            icon: home.join(".local/share/icons/hicolor/scalable/apps/test.svg"),
            icon_svg: "<svg/>".into(),
        };
        let plan = Plan {
            distro: Distro::Arch,
            tasks: vec![PlannedTask {
                id: "app.test".into(),
                title: "Test".into(),
                steps: vec![Step::SignedAppImage {
                    package: "test".into(),
                    manifest_url: "https://example.test/linux-release.json".into(),
                    public_key: key,
                    install,
                }],
                reboot: false,
                requires: Vec::new(),
                required_by: None,
            }],
        };
        let user = User { name: "burce".into(), uid: 1000, home: home.clone() };
        let log = Rc::new(RefCell::new(Vec::new()));
        let runner =
            FakeRunner { log: Rc::clone(&log), respond: release_responder(manifest, b"genuine package") };
        let mut executor = Executor::new(f.root(), &sys, runner, Some(user)).unwrap();
        let mut events = Vec::new();
        executor.execute(&plan, &mut |e| events.push(e));
        let commands: Vec<String> = log.borrow().iter().map(Cmd::display).collect();
        let run = Run { summary: Summary { ok: true, reboot: false }, events, commands };
        assert_eq!(run.outcome("app.test").0, Outcome::Done, "{:#?}", run.events);

        let cache = f.root().join("var/cache/bootcraft/downloads");
        let root = f.root().display().to_string();
        assert_eq!(
            run.commands[2..],
            [
                format!(
                    "$ install -D -m 755 {}/Test-2.0.0-x86_64.AppImage {root}/home/burce/Applications/Test.AppImage  (as burce)",
                    cache.display()
                ),
                format!(
                    "$ install -D -m 644 {}/Test-2.0.0-x86_64.AppImage.desktop {root}/home/burce/.local/share/applications/test.desktop  (as burce)",
                    cache.display()
                ),
                format!(
                    "$ install -D -m 644 {}/Test-2.0.0-x86_64.AppImage.svg {root}/home/burce/.local/share/icons/hicolor/scalable/apps/test.svg  (as burce)",
                    cache.display()
                ),
            ]
        );
        assert_eq!(fs::read_to_string(cache.join("Test-2.0.0-x86_64.AppImage.svg")).unwrap(), "<svg/>");
        assert!(!run.commands.iter().any(|c| c.contains("pacman")), "no package manager: {:?}", run.commands);
    }

    #[test]
    fn tampered_download_is_rejected_and_deleted() {
        let f = arch_box();
        let sys = f.detect("KDE");
        let run = run(&f, &sys, b"evil package", NO_CONFLICTS);
        let (outcome, message) = run.outcome("app.testpkg");
        assert_eq!(outcome, Outcome::Failed);
        assert!(message.unwrap().contains("does not match the signed checksum"));
        assert!(!run.commands.iter().any(|c| c.contains("pacman")));
        assert!(!f.root().join("var/cache/bootcraft/downloads/testpkg-2.0.0-1-x86_64.pkg.tar.zst").exists());
    }

    #[test]
    fn a_declared_predecessor_is_replaced() {
        let f = arch_box();
        f.package("arctic-comfyui-helper", &[]);
        let sys = f.detect("KDE");
        let info = "Name            : testpkg\nConflicts With  : arctic-comfyui-helper\nReplaces        : arctic-comfyui-helper\n";
        let run = run(&f, &sys, b"genuine package", info);
        assert_eq!(run.outcome("app.testpkg").0, Outcome::Done, "{:#?}", run.events);
        let install = run.commands.last().unwrap();
        assert!(install.starts_with("$ pacman -U --needed --noconfirm --ask=4 "), "{install}");
    }

    #[test]
    fn an_undeclared_conflict_stops_before_pacman() {
        let f = arch_box();
        f.package("someone-elses-app", &[]);
        let sys = f.detect("KDE");
        let info = "Name            : testpkg\nConflicts With  : someone-elses-app\nReplaces        : None\n";
        let run = run(&f, &sys, b"genuine package", info);
        let (outcome, message) = run.outcome("app.testpkg");
        assert_eq!(outcome, Outcome::Failed);
        assert!(message.unwrap().contains("conflicts with the installed someone-elses-app"));
        assert!(!run.commands.iter().any(|c| c.starts_with("$ pacman -U")));
    }
}

#[test]
fn an_existing_aur_helper_is_kept() {
    let f = arch_box();
    f.package("base-devel", &[]).package("git", &[]).package("paru", &[]);
    let sys = f.detect("KDE");
    let run = execute(&f, &sys, &["arch.aur-helper"], None, succeed());
    assert_eq!(run.outcome("arch.aur-helper").0, Outcome::AlreadyDone);
    // The required system update still runs; no yay is built or installed.
    assert!(
        !run.commands.iter().any(|c| c.contains("yay-bin") || c.contains("makepkg")),
        "no second AUR helper: {:?}",
        run.commands
    );
}

#[test]
fn optional_program_configs_are_not_created() {
    // The Mint look edits lightdm.conf only if LightDM is there.
    let f = Fixture::new(ARCH);
    f.kernel("7.2.11-arch1-1", "linux");
    let sys = f.detect("X-Cinnamon");
    let task = catalog(&sys).tasks.into_iter().find(|t| t.id == "desktop.cinnamon-mint").unwrap();
    let edit = task.steps.iter().find(|s| matches!(s, bootcraft_core::Step::Edit { .. })).unwrap();
    let probe = bootcraft_core::probe::Probe::new(f.root(), bootcraft_core::system::Distro::Arch);
    assert_eq!(probe.status(std::slice::from_ref(edit)), bootcraft_core::probe::Status::Done);
}

#[test]
fn failures_quote_the_tools_own_error() {
    let f = arch_box();
    let sys = f.detect("KDE");
    let respond: Respond =
        Box::new(|_| (false, "resolving dependencies...\nerror: target not found: nope\n".to_owned()));
    let run = execute(&f, &sys, &["system.update"], None, respond);
    assert_eq!(run.outcome("system.update").1.as_deref(), Some("pacman failed: target not found: nope"));
}
