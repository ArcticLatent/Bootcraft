//! Fedora, Debian, Ubuntu and Mint catalogs: every task and every pack app
//! can be planned, and the recommended plans read as expected.

mod common;

use std::collections::HashSet;

use bootcraft_core::catalog::{PACKS, catalog};
use bootcraft_core::plan::recommended;
use bootcraft_core::{Category, Preset, Selection, Step, System, plan};
use common::Fixture;

const FEDORA_44: &str = "ID=fedora\nVERSION_ID=44\nPRETTY_NAME=\"Fedora Linux 44 (Workstation Edition)\"\n";
const DEBIAN_13: &str = "ID=debian\nVERSION_ID=\"13\"\nPRETTY_NAME=\"Debian GNU/Linux 13 (trixie)\"\n";
const UBUNTU_2604: &str =
    "ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"26.04\"\nPRETTY_NAME=\"Ubuntu 26.04 LTS\"\n";
const MINT_22: &str =
    "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\nVERSION_ID=\"22.2\"\nPRETTY_NAME=\"Linux Mint 22.2\"\n";

const RTX_4070: (u16, u16) = (0x10de, 0x2786);
const GTX_1080: (u16, u16) = (0x10de, 0x1b80);
const RX_7900: (u16, u16) = (0x1002, 0x744c);

fn system(os: &str, gpu: (u16, u16), desktop: &str) -> (Fixture, System) {
    let f = Fixture::new(os);
    f.gpu("0000:01:00.0", gpu.0, gpu.1, true);
    let sys = f.detect(desktop);
    (f, sys)
}

fn tasks_plan(sys: &System, ids: Vec<String>) -> String {
    let selection = Selection { tasks: ids, ..Selection::default() };
    plan(sys, &catalog(sys), &selection).unwrap().render()
}

#[test]
fn every_task_and_pack_app_plans_on_every_distro() {
    for os in [FEDORA_44, DEBIAN_13, UBUNTU_2604, MINT_22] {
        for gpu in [RTX_4070, GTX_1080, RX_7900] {
            for desktop in ["GNOME", "KDE", "X-Cinnamon"] {
                let (_f, sys) = system(os, gpu, desktop);
                let catalog = catalog(&sys);
                let ids: Vec<String> = catalog.tasks.iter().map(|t| t.id.to_owned()).collect();
                let unique: HashSet<_> = ids.iter().collect();
                assert_eq!(unique.len(), ids.len(), "duplicate task IDs on {}", sys.os.pretty_name);
                for task in &catalog.tasks {
                    assert!(!task.steps.is_empty(), "{} has no steps", task.id);
                    if task.category == Category::Debloat {
                        assert_eq!(task.preset, Preset::Optional, "{} must be opt-in", task.id);
                    }
                    plan(
                        &sys,
                        &catalog,
                        &Selection { tasks: vec![task.id.to_owned()], ..Selection::default() },
                    )
                    .unwrap_or_else(|e| panic!("{} on {}: {e}", task.id, sys.os.pretty_name));
                }
                // Everything at once, and every pack's apps from either source.
                let mut everything =
                    Selection::from_packs(&catalog, &["creative", "development", "gaming"]).unwrap();
                everything.tasks = ids;
                plan(&sys, &catalog, &everything).unwrap_or_else(|e| panic!("{}: {e}", sys.os.pretty_name));
                for pack in PACKS {
                    for (app, _) in pack.apps {
                        let app = catalog.app(app).unwrap();
                        for source in
                            [bootcraft_core::apps::Source::Flatpak, bootcraft_core::apps::Source::Native]
                        {
                            if !app.has(source) {
                                continue;
                            }
                            let pick = bootcraft_core::AppPick { id: app.id.into(), source: Some(source) };
                            plan(&sys, &catalog, &Selection { apps: vec![pick], ..Selection::default() })
                                .unwrap_or_else(|e| {
                                    panic!("{} from {source:?} on {}: {e}", app.id, sys.os.pretty_name)
                                });
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn nvidia_branches_per_distro() {
    let driver = |os, gpu| {
        let (_f, sys) = system(os, gpu, "GNOME");
        let catalog = catalog(&sys);
        let task = catalog.task("gpu.nvidia").unwrap().clone();
        task.steps
    };
    // Fedora: RPM Fusion's akmods, the 580xx branch for legacy cards.
    assert!(
        matches!(&driver(FEDORA_44, RTX_4070)[0], Step::Install { packages, .. } if packages[0] == "akmod-nvidia")
    );
    assert!(
        matches!(&driver(FEDORA_44, GTX_1080)[0], Step::Install { packages, .. } if packages[0] == "akmod-nvidia-580xx")
    );
    // Debian: NVIDIA's repository (pinned keyring) for current cards, Debian's 550 for legacy ones.
    assert!(matches!(&driver(DEBIAN_13, RTX_4070)[0], Step::Download { sha256, .. } if sha256.len() == 64));
    assert!(driver(DEBIAN_13, GTX_1080).iter().any(
        |s| matches!(s, Step::Install { packages, .. } if packages.contains(&"nvidia-kernel-dkms".to_owned()))
    ));
    // Ubuntu and Mint: ubuntu-drivers decides the branch at run time.
    assert!(driver(UBUNTU_2604, RTX_4070).contains(&Step::UbuntuDriver { open: true, max_branch: None }));
    assert!(driver(MINT_22, GTX_1080).contains(&Step::UbuntuDriver { open: false, max_branch: Some(580) }));
}

#[test]
fn snap_replacement_is_ubuntu_only_and_opt_in() {
    let (_u, ubuntu) = system(UBUNTU_2604, RTX_4070, "GNOME");
    let catalog_u = catalog(&ubuntu);
    let snap = catalog_u.task("ubuntu.replace-snap").unwrap();
    assert_eq!(snap.preset, Preset::Optional);
    assert!(snap.steps.contains(&Step::RemoveSnaps));
    let (_m, mint) = system(MINT_22, RTX_4070, "X-Cinnamon");
    assert!(catalog(&mint).task("ubuntu.replace-snap").is_none());
}

#[test]
fn debian_offers_sudo_for_the_user() {
    let (_f, sys) = system(DEBIAN_13, RX_7900, "GNOME");
    let catalog = catalog(&sys);
    let sudo = catalog.task("debian.sudo").unwrap();
    assert!(sudo.steps.contains(&Step::AddToGroup { user: "burce".into(), group: "sudo".into() }));
}

#[test]
fn snapshot_recommended_plans() {
    for (name, os, gpu, desktop) in [
        ("fedora_kde_rtx4070", FEDORA_44, RTX_4070, "KDE"),
        ("fedora_gnome_amd", FEDORA_44, RX_7900, "GNOME"),
        ("debian_gnome_rtx4070", DEBIAN_13, RTX_4070, "GNOME"),
        ("debian_kde_gtx1080", DEBIAN_13, GTX_1080, "KDE"),
        ("ubuntu_gnome_rtx4070", UBUNTU_2604, RTX_4070, "ubuntu:GNOME"),
        ("mint_cinnamon_gtx1080", MINT_22, GTX_1080, "X-Cinnamon"),
    ] {
        let (_f, sys) = system(os, gpu, desktop);
        let ids = recommended(&catalog(&sys).tasks);
        insta::assert_snapshot!(name, tasks_plan(&sys, ids));
    }
}
