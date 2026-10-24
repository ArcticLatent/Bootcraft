mod common;

use std::collections::HashSet;
use std::fmt::Write as _;

use bootcraft_core::catalog::catalog;
use bootcraft_core::plan::recommended;
use bootcraft_core::probe::{Probe, Status};
use bootcraft_core::system::{Desktop, Distro, NvidiaGen, SecureBoot};
use bootcraft_core::{Category, Plan, PlanError, Preset, Selection, Step, System, Task, plan};
use common::{ARCH, Fixture, PACMAN_CONF};

const RTX_4070: u16 = 0x2786;
const GTX_1080: u16 = 0x1b80;
const GTX_680: u16 = 0x1180;
const RX_7900: u16 = 0x744c;

fn kde_rtx4070() -> Fixture {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, RTX_4070, true).kernel("7.2.11-arch1-1", "linux");
    f
}

fn gnome_gtx1080_with_lts() -> Fixture {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, GTX_1080, true)
        .kernel("7.2.11-arch1-1", "linux")
        .kernel("6.18.58-1-lts", "linux-lts")
        .secure_boot(true);
    f
}

fn cinnamon_amd_with_sshd() -> Fixture {
    let f = Fixture::new(ARCH);
    f.gpu("0000:03:00.0", 0x1002, RX_7900, true)
        .kernel("7.2.11-arch1-1", "linux")
        .file("usr/lib/systemd/system/sshd.service", "[Unit]\n");
    f
}

fn hybrid_laptop_custom_kernel() -> Fixture {
    let f = Fixture::new(ARCH);
    f.gpu("0000:00:02.0", 0x8086, 0xa7a0, true)
        .gpu("0000:01:00.0", 0x10de, RTX_4070, false)
        .kernel("7.3.0-273-tkg-eevdf", "linux73-tkg-eevdf")
        .kernel("7.2.11-zen1-1-zen", "linux-zen");
    f
}

/// Catalog overview: what the GUI's task screen would list.
fn summarize(tasks: &[Task]) -> String {
    let mut out = String::new();
    for task in tasks {
        let tick = if task.preset == Preset::Recommended { "x" } else { " " };
        let _ = writeln!(out, "[{tick}] {:<10} {} — {}", task.category.title(), task.id, task.title);
        for note in &task.notes {
            let _ = writeln!(out, "      note: {note}");
        }
    }
    out
}

fn tasks(sys: &System) -> Vec<Task> {
    catalog(sys).tasks
}

fn plan_tasks(sys: &System, ids: &[impl AsRef<str>]) -> Result<Plan, PlanError> {
    let selection =
        Selection { tasks: ids.iter().map(|i| i.as_ref().to_owned()).collect(), ..Selection::default() };
    plan(sys, &catalog(sys), &selection)
}

fn recommended_plan(sys: &System) -> String {
    plan_tasks(sys, &recommended(&tasks(sys))).unwrap().render()
}

fn all_ids(tasks: &[Task]) -> Vec<&'static str> {
    tasks.iter().map(|t| t.id).collect()
}

#[test]
fn detects_hardware_and_session() {
    let f = hybrid_laptop_custom_kernel();
    f.file(
        "usr/share/hwdata/pci.ids",
        "10de  NVIDIA Corporation\n\t2786  AD104 [GeForce RTX 4070]\n8086  Intel Corporation\n\ta7a0  Raptor Lake-P [Iris Xe Graphics]\n",
    );
    let sys = f.detect("KDE");
    assert_eq!(sys.distro(), Some(Distro::Arch));
    assert_eq!(sys.desktop, Desktop::Kde);
    assert_eq!(sys.gpus.len(), 2);
    assert_eq!(sys.gpus[0].name.as_deref(), Some("Raptor Lake-P [Iris Xe Graphics]"));
    assert!(sys.gpus[0].boot_vga);
    assert_eq!(sys.gpus[1].name.as_deref(), Some("AD104 [GeForce RTX 4070]"));
    assert_eq!(sys.nvidia, Some(NvidiaGen::Modern));
    assert_eq!(sys.arch_kernels, ["linux-zen", "linux73-tkg-eevdf"]);
    assert_eq!(sys.secure_boot, SecureBoot::NotEfi);
    assert!(!sys.ssh_server);
    assert_eq!(sys.memory_bytes, Some(65_536_000 * 1024));
    assert!(sys.disk_free_bytes.is_some());
    assert!(sys.warnings().is_empty());
}

#[test]
fn oldest_nvidia_card_decides_the_branch() {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, RTX_4070, true).gpu("0000:02:00.0", 0x10de, GTX_1080, false);
    assert_eq!(f.detect("GNOME").nvidia, Some(NvidiaGen::Legacy580));
}

#[test]
fn kepler_gets_a_warning_and_no_driver_task() {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, GTX_680, true);
    let sys = f.detect("GNOME");
    assert_eq!(sys.nvidia, Some(NvidiaGen::Unsupported));
    assert!(sys.warnings().iter().any(|w| w.contains("nouveau")));
    let ids = all_ids(&tasks(&sys));
    assert!(!ids.contains(&"gpu.nvidia") && !ids.contains(&"gpu.nvidia-vaapi"));
}

#[test]
fn derivatives_are_refused() {
    let f = Fixture::new("ID=manjaro\nID_LIKE=arch\nPRETTY_NAME=\"Manjaro Linux\"\n");
    let sys = f.detect("KDE");
    assert!(tasks(&sys).is_empty());
    assert_eq!(
        plan_tasks(&sys, &["system.update"]),
        Err(PlanError::UnsupportedSystem("Manjaro Linux".into()))
    );
}

#[test]
fn catalog_invariants_hold_for_every_fixture() {
    for (fixture, desktop) in [
        (kde_rtx4070(), "KDE"),
        (gnome_gtx1080_with_lts(), "GNOME"),
        (cinnamon_amd_with_sshd(), "X-Cinnamon"),
        (hybrid_laptop_custom_kernel(), "Hyprland"),
    ] {
        let sys = fixture.detect(desktop);
        let tasks = tasks(&sys);
        let ids = all_ids(&tasks);
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len(), "duplicate IDs: {ids:?}");
        for task in &tasks {
            assert!(!task.steps.is_empty(), "{} has no steps", task.id);
            for req in &task.requires {
                assert!(ids.contains(req), "{} requires missing {req}", task.id);
            }
            if task.category == Category::Debloat {
                assert_eq!(task.preset, Preset::Optional, "{} must be opt-in", task.id);
            }
        }
        // Every task can be planned, alone and all together.
        plan_tasks(&sys, &ids).unwrap();
        for id in &ids {
            plan_tasks(&sys, &[id]).unwrap();
        }
    }
}

#[test]
fn requirements_are_pulled_in_and_ordered() {
    let sys = kde_rtx4070().detect("KDE");
    let plan = plan_tasks(&sys, &["arch.aur-helper"]).unwrap();
    let order: Vec<_> = plan.tasks.iter().map(|t| (t.id.as_str(), t.required_by.as_deref())).collect();
    assert_eq!(
        order,
        [
            ("system.update", Some("arch.build-tools")),
            ("arch.build-tools", Some("arch.aur-helper")),
            ("arch.aur-helper", None),
        ]
    );
}

#[test]
fn unknown_task_is_an_error() {
    let sys = kde_rtx4070().detect("KDE");
    assert_eq!(plan_tasks(&sys, &["rm-rf"]), Err(PlanError::UnknownTask("rm-rf".into())));
}

#[test]
fn prebuilt_modules_cover_linux_and_lts() {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, RTX_4070, true)
        .kernel("7.2.11-arch1-1", "linux")
        .kernel("6.18.58-1-lts", "linux-lts");
    let sys = f.detect("GNOME");
    let tasks = tasks(&sys);
    let driver = tasks.iter().find(|t| t.id == "gpu.nvidia").unwrap();
    assert!(matches!(
        &driver.steps[0],
        Step::Install { packages, .. } if packages == &["nvidia-open", "nvidia-open-lts"]
    ));
}

#[test]
fn custom_driver_build_counts_as_installed() {
    let f = hybrid_laptop_custom_kernel();
    f.package("nvidia-open-dkms-tkg", &["nvidia-open", "NVIDIA-MODULE"])
        .package("nvidia-utils-tkg", &["nvidia-utils=615.78.08", "vulkan-driver"])
        .package("lib32-nvidia-utils-tkg", &["lib32-nvidia-utils=615.78.08"]);
    let sys = f.detect("KDE");
    let tasks = tasks(&sys);
    let probe = Probe::new(f.root(), Distro::Arch);
    let driver = tasks.iter().find(|t| t.id == "gpu.nvidia").unwrap();
    assert_eq!(probe.status(&driver.steps), Status::Done);
}

#[test]
fn probe_tracks_config_edits_and_packages() {
    let f = kde_rtx4070();
    f.file("etc/pacman.conf", PACMAN_CONF).package("base-devel", &[]).package("git", &[]);
    let sys = f.detect("KDE");
    let tasks = tasks(&sys);
    let probe = Probe::new(f.root(), Distro::Arch);
    let status = |id| probe.status(&tasks.iter().find(|t| t.id == id).unwrap().steps);

    assert_eq!(status("arch.multilib"), Status::Pending);
    assert_eq!(status("arch.build-tools"), Status::Done);
    assert_eq!(status("system.update"), Status::Unknown);
    // Nothing from the list is installed, so there is nothing to remove.
    assert_eq!(status("debloat.libreoffice"), Status::Done);

    f.package("libreoffice-fresh", &[]);
    let probe = Probe::new(f.root(), Distro::Arch);
    let libreoffice = tasks.iter().find(|t| t.id == "debloat.libreoffice").unwrap();
    assert_eq!(probe.status(&libreoffice.steps), Status::Pending);

    let multilib = tasks.iter().find(|t| t.id == "arch.multilib").unwrap();
    let Step::Edit { edit, .. } = &multilib.steps[0] else { panic!("expected an edit") };
    f.file("etc/pacman.conf", &edit.apply(PACMAN_CONF));
    assert_eq!(probe.status(&multilib.steps), Status::Done);
}

#[test]
fn snapshot_kde_rtx4070() {
    let sys = kde_rtx4070().detect("KDE");
    insta::assert_snapshot!("kde_rtx4070_catalog", summarize(&tasks(&sys)));
    insta::assert_snapshot!("kde_rtx4070_plan", recommended_plan(&sys));
}

#[test]
fn snapshot_gnome_gtx1080_secure_boot() {
    let sys = gnome_gtx1080_with_lts().detect("GNOME");
    insta::assert_snapshot!("gnome_gtx1080_catalog", summarize(&tasks(&sys)));
    insta::assert_snapshot!("gnome_gtx1080_plan", recommended_plan(&sys));
}

#[test]
fn snapshot_cinnamon_amd_everything() {
    let sys = cinnamon_amd_with_sshd().detect("X-Cinnamon");
    let tasks = tasks(&sys);
    insta::assert_snapshot!("cinnamon_amd_catalog", summarize(&tasks));
    let everything = plan_tasks(&sys, &all_ids(&tasks)).unwrap().render();
    insta::assert_snapshot!("cinnamon_amd_all_tasks_plan", everything);
}

#[test]
fn snapshot_hybrid_custom_kernel() {
    let sys = hybrid_laptop_custom_kernel().detect("Hyprland");
    insta::assert_snapshot!("hybrid_custom_kernel_catalog", summarize(&tasks(&sys)));
}
