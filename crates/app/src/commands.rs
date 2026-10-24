//! The commands QML can invoke. Each runs on a worker thread and returns JSON.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bootcraft_core::apps::{Preference, Via};
use bootcraft_core::catalog::{PACKS, catalog};
use bootcraft_core::detect::{Env, detect};
use bootcraft_core::probe::{Probe, Status};
use bootcraft_core::protocol::NvidiaChoice;
use bootcraft_core::system::{Distro, GpuVendor, System};
use bootcraft_core::{AppPick, Selection, plan};
use serde_json::{Value, json};

pub fn dispatch(command: &str, args: &Value) -> Result<Value, String> {
    match command {
        "system" => Ok(system_info(&system(args))),
        "catalog" => Ok(catalog_info(&system(args), preference(args))),
        "plan" => plan_info(&system(args), args),
        "export_plan" => export_plan(&system(args), args),
        "download_size" => Ok(crate::sizes::download_size(&system(args), args)),
        "apply" => crate::helper::start(args),
        "cancel" => crate::helper::cancel(),
        "app_icons" => Ok(crate::icons::fetch(args)),
        "reboot" => reboot(),
        "check_update" => crate::update::check(),
        "install_update" => crate::update::install(),
        other => Err(format!("unknown command `{other}`")),
    }
}

/// Detect, then apply the user's NVIDIA override from Settings.
fn system(args: &Value) -> System {
    let mut sys = detect(Path::new("/"), &Env::from_process());
    let choice: Option<NvidiaChoice> = serde_json::from_value(args["nvidia"].clone()).ok();
    if let Some(choice) = choice {
        choice.apply(&mut sys);
    }
    sys
}

fn preference(args: &Value) -> Preference {
    serde_json::from_value(args["preference"].clone()).unwrap_or_default()
}

fn selection(args: &Value) -> Selection {
    Selection {
        tasks: serde_json::from_value(args["tasks"].clone()).unwrap_or_default(),
        apps: serde_json::from_value::<Vec<AppPick>>(args["apps"].clone()).unwrap_or_default(),
        preference: preference(args),
    }
}

#[allow(clippy::cast_precision_loss)] // Display only.
fn gib(bytes: u64) -> f64 {
    bytes as f64 / f64::from(1u32 << 30)
}

/// RAM is sold in round sizes and the kernel reports a little less (firmware,
/// integrated graphics), so show the size people bought.
fn memory_label(bytes: u64) -> String {
    const SIZES: &[u64] = &[2, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024];
    let size = gib(bytes);
    #[allow(clippy::cast_precision_loss)]
    let rounded = SIZES.iter().find(|&&s| s as f64 >= size).copied();
    match rounded {
        Some(gb) => format!("{gb} GB memory"),
        None => format!("{size:.0} GB memory"),
    }
}

fn disk_label(bytes: u64) -> String {
    let gb = gib(bytes);
    if gb >= 1024.0 { format!("{:.1} TB free", gb / 1024.0) } else { format!("{gb:.0} GB free") }
}

fn system_info(sys: &System) -> Value {
    let (logo, logo_color) = match sys.distro() {
        Some(Distro::Fedora) => ("fedora", "#51A2DA"),
        Some(Distro::Arch) => ("archlinux", "#1793D1"),
        Some(Distro::Ubuntu) => ("ubuntu", "#E95420"),
        Some(Distro::Mint) => ("linuxmint", "#86BE43"),
        Some(Distro::Debian) => ("debian", "#A81D33"),
        None => ("", ""),
    };
    // The graphics the user cares about: a discrete card over the integrated one.
    let main_gpu = [GpuVendor::Nvidia, GpuVendor::Amd, GpuVendor::Intel]
        .iter()
        .find(|vendor| sys.gpus.iter().any(|g| g.vendor == **vendor));
    let (graphics, gpu_logo, gpu_color) = match main_gpu {
        Some(GpuVendor::Nvidia) => ("NVIDIA graphics", "nvidia", "#76B900"),
        Some(GpuVendor::Amd) => ("AMD graphics", "amd", "#ED1C24"),
        Some(GpuVendor::Intel) => ("Intel graphics", "intel", "#0071C5"),
        _ => ("No GPU detected", "", ""),
    };
    json!({
        "distro": sys.distro(),
        "name": sys.distro().map_or_else(|| sys.os.pretty_name.clone(), |d| d.name().to_owned()),
        "prettyName": sys.os.pretty_name,
        "supported": sys.distro().is_some(),
        "versionSupported": sys.os.version_supported(),
        "logo": logo,
        "logoColor": logo_color,
        "graphics": graphics,
        "gpuLogo": gpu_logo,
        "gpuColor": gpu_color,
        "gpus": sys.gpus.iter().map(|g| json!({
            "name": g.name,
            "vendor": g.vendor,
            "driver": g.nvidia_gen().map(|n| n.label()),
        })).collect::<Vec<_>>(),
        "nvidia": sys.nvidia,
        "memory": sys.memory_bytes.map(memory_label),
        "diskFree": sys.disk_free_bytes.map(disk_label),
        "desktop": sys.desktop.label(),
        "session": sys.session,
        "kernel": sys.kernel_release,
        "secureBoot": sys.secure_boot,
        "warnings": sys.warnings(),
    })
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Done => "done",
        Status::Pending => "pending",
        Status::Unknown => "unknown",
    }
}

fn catalog_info(sys: &System, preference: Preference) -> Value {
    let catalog = catalog(sys);
    let probe = sys.distro().map(|d| Probe::new(Path::new("/"), d));
    let status = |steps: &[_]| probe.as_ref().map_or("unknown", |p| status_name(p.status(steps)));

    let packs: Vec<Value> = PACKS
        .iter()
        .map(|p| {
            json!({
                "id": p.id, "title": p.title, "subtitle": p.subtitle,
                "apps": p.apps.iter().map(|(id, ticked)| json!({"id": id, "ticked": ticked})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let tasks: Vec<Value> = catalog
        .tasks
        .iter()
        .map(|t| {
            json!({
                "id": t.id, "title": t.title, "description": t.description,
                "category": t.category, "categoryTitle": t.category.title(),
                "recommended": t.preset == bootcraft_core::Preset::Recommended,
                "notes": t.notes, "requires": t.requires, "status": status(&t.steps),
            })
        })
        .collect();
    let apps: Vec<Value> = catalog
        .apps
        .iter()
        .map(|a| {
            let choice = a.choose(sys, preference, None);
            let installed = choice.and_then(|c| a.steps(c.source)).map_or("unknown", |steps| status(&steps));
            json!({
                "id": a.id, "name": a.name, "summary": a.summary,
                "section": a.section, "sectionTitle": a.section.title(),
                "flatpak": a.flatpak,
                "native": a.native.as_ref().map(|n| json!({
                    "via": match n.via {
                        Via::Repo => "repo",
                        Via::RpmFusion => "rpm-fusion",
                        Via::Aur => "aur",
                        Via::SignedRelease { .. } => "signed-release",
                    },
                    "packages": n.packages,
                })),
                "notes": a.notes,
                "icon": a.icon_svg.map(|svg| format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))),
                "available": a.available(),
                "choice": choice,
                "status": installed,
            })
        })
        .collect();
    json!({ "packs": packs, "tasks": tasks, "apps": apps })
}

/// The plan for the current selection: what the summary card counts and
/// what Start setup will run.
fn plan_info(sys: &System, args: &Value) -> Result<Value, String> {
    let catalog = catalog(sys);
    let plan = plan(sys, &catalog, &selection(args)).map_err(|e| e.to_string())?;
    let apps = plan.tasks.iter().filter(|t| t.id.starts_with("app.")).count();
    Ok(json!({
        "tasks": plan.tasks.iter().map(|t| json!({
            "id": t.id,
            "title": t.title,
            "isApp": t.id.starts_with("app."),
            "requiredBy": t.required_by,
            "steps": t.steps.iter().map(|s| s.describe(plan.distro)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "appCount": apps,
        "systemCount": plan.tasks.len() - apps,
        "reboot": plan.reboot_required(),
        "text": plan.render(),
    }))
}

/// The plan as a runnable bash script (see `bootcraft_core::script`).
fn export_plan(sys: &System, args: &Value) -> Result<Value, String> {
    use std::os::unix::fs::PermissionsExt;

    let path = args["path"].as_str().ok_or("no file chosen")?;
    let catalog = catalog(sys);
    let plan = plan(sys, &catalog, &selection(args)).map_err(|e| e.to_string())?;
    let date = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let mut resolve = bootcraft_core::script::resolve_signed(plan.distro);
    let script = bootcraft_core::script::to_script(&plan, Path::new("/"), &date, &mut resolve);
    std::fs::write(path, script).map_err(|e| format!("{path}: {e}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(json!({ "path": path }))
}

fn reboot() -> Result<Value, String> {
    // logind lets the active session's user reboot; polkit asks otherwise.
    let status = std::process::Command::new("systemctl")
        .arg("reboot")
        .status()
        .map_err(|e| format!("could not run systemctl: {e}"))?;
    if status.success() { Ok(Value::Null) } else { Err("the reboot was not allowed".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bootcraft_core::apps::Source;

    #[test]
    fn memory_rounds_up_to_the_size_people_bought() {
        let gib = |g: f64| (g * f64::from(1u32 << 30)) as u64;
        assert_eq!(memory_label(gib(93.4)), "96 GB memory");
        assert_eq!(memory_label(gib(15.5)), "16 GB memory");
        assert_eq!(memory_label(gib(31.2)), "32 GB memory");
    }

    #[test]
    fn disk_labels() {
        assert_eq!(disk_label(128 << 30), "128 GB free");
        assert_eq!(disk_label(1536 << 30), "1.5 TB free");
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(dispatch("rm", &Value::Null).is_err());
    }

    #[test]
    fn source_choice_serializes_for_qml() {
        let value = serde_json::to_value(Source::Flatpak).unwrap();
        assert_eq!(value, "flatpak");
    }
}
