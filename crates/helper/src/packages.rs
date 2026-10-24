//! Every package name a distribution's catalog can install, for checking
//! them against its real repositories in CI (scripts/check-packages.sh).

use std::collections::BTreeSet;

use bootcraft_core::Step;
use bootcraft_core::apps::Via;
use bootcraft_core::catalog::catalog;
use bootcraft_core::system::{Desktop, Gpu, GpuVendor, NvidiaGen, System};

/// `repo <name>`, `aur <name>` and `group <name>` lines, sorted and unique.
/// The catalog is built for every GPU and desktop variant, since those
/// change which packages appear.
pub fn all(sys: &System) -> BTreeSet<String> {
    let mut lines = BTreeSet::new();
    let gpu = |vendor, vendor_id, device_id| Gpu {
        slot: String::new(),
        vendor,
        vendor_id,
        device_id,
        name: None,
        boot_vga: false,
    };
    let gpu_sets = [
        (vec![gpu(GpuVendor::Nvidia, 0x10de, 0x2786)], Some(NvidiaGen::Modern)),
        (vec![gpu(GpuVendor::Nvidia, 0x10de, 0x1b80)], Some(NvidiaGen::Legacy580)),
        (vec![gpu(GpuVendor::Amd, 0x1002, 0x744c), gpu(GpuVendor::Intel, 0x8086, 0xa7a0)], None),
    ];
    for (gpus, nvidia) in gpu_sets {
        for desktop in [Desktop::Gnome, Desktop::Kde, Desktop::Cinnamon] {
            let mut variant = sys.clone();
            variant.gpus.clone_from(&gpus);
            variant.nvidia = nvidia;
            variant.desktop = desktop;
            variant.user = Some("user".into());
            let catalog = catalog(&variant);
            for task in &catalog.tasks {
                for step in &task.steps {
                    match step {
                        Step::Install { packages, .. } => add(&mut lines, "repo", packages),
                        Step::AurInstall { packages, .. } => add(&mut lines, "aur", packages),
                        Step::Swap { to, .. } => add(&mut lines, "repo", std::slice::from_ref(to)),
                        _ => {}
                    }
                }
            }
            for app in &catalog.apps {
                let Some(native) = &app.native else { continue };
                match native.via {
                    Via::Repo | Via::RpmFusion => add(&mut lines, "repo", &native.packages),
                    Via::Aur => add(&mut lines, "aur", &native.packages),
                    Via::SignedRelease { .. } => {}
                }
            }
        }
    }
    lines
}

fn add(lines: &mut BTreeSet<String>, source: &str, packages: &[String]) {
    for package in packages {
        if package.contains('/') {
            // URLs and local files (release packages, downloaded keyrings).
            continue;
        }
        match package.strip_prefix('@') {
            Some(group) => lines.insert(format!("group {group}")),
            None => lines.insert(format!("{source} {package}")),
        };
    }
}
