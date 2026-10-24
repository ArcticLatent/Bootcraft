mod common;

use std::collections::HashSet;

use bootcraft_core::apps::{Preference, Source, Via};
use bootcraft_core::catalog::{PACKS, catalog, pack};
use bootcraft_core::{AppPick, PlanError, Selection, Step, System, plan};
use common::{ARCH, Fixture};

const UBUNTU_2404: &str =
    "ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04.3 LTS\"\n";
const UBUNTU_2604: &str =
    "ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"26.04\"\nPRETTY_NAME=\"Ubuntu 26.04 LTS\"\n";
const DEBIAN_13: &str = "ID=debian\nVERSION_ID=\"13\"\nPRETTY_NAME=\"Debian GNU/Linux 13 (trixie)\"\n";
const FEDORA_44: &str = "ID=fedora\nVERSION_ID=44\nPRETTY_NAME=\"Fedora Linux 44 (Workstation Edition)\"\n";
const MINT_22: &str =
    "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\nVERSION_ID=\"22.2\"\nPRETTY_NAME=\"Linux Mint 22.2\"\n";

fn system(os_release: &str, gpu: (u16, u16)) -> (Fixture, System) {
    let f = Fixture::new(os_release);
    f.gpu("0000:01:00.0", gpu.0, gpu.1, true).kernel("7.2.11-arch1-1", "linux");
    let sys = f.detect("KDE");
    (f, sys)
}

const RTX_4070: (u16, u16) = (0x10de, 0x2786);
const RX_7900: (u16, u16) = (0x1002, 0x744c);

fn choose(sys: &System, id: &str, preference: Preference, explicit: Option<Source>) -> Option<Source> {
    catalog(sys).app(id).unwrap().choose(sys, preference, explicit).map(|c| c.source)
}

#[test]
fn packs_are_consistent_on_every_distribution() {
    let essentials = pack("essentials").unwrap();
    assert!(essentials.apps.is_empty(), "Essentials installs no apps");
    for os in [ARCH, UBUNTU_2404, UBUNTU_2604, DEBIAN_13, FEDORA_44, MINT_22] {
        let (_f, sys) = system(os, RTX_4070);
        let catalog = catalog(&sys);
        let ids: Vec<_> = catalog.apps.iter().map(|a| a.id).collect();
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len(), "duplicate app IDs");
        for pack in PACKS {
            let mut seen = HashSet::new();
            for (app, _) in pack.apps {
                assert!(catalog.app(app).is_some(), "{} lists unknown app {app}", pack.id);
                assert!(seen.insert(app), "{} lists {app} twice", pack.id);
            }
        }
        for app in &catalog.apps {
            assert!(!app.summary.is_empty() && !app.name.is_empty(), "{}", app.id);
            if let (Some(native), Some(distro)) = (&app.native, sys.distro()) {
                assert!(!native.packages.is_empty(), "{} on {distro:?}", app.id);
                if native.via == Via::Aur {
                    assert_eq!(distro, bootcraft_core::system::Distro::Arch, "{} uses the AUR", app.id);
                }
            }
        }
    }
}

#[test]
fn workflow_defaults_match_the_agreed_picks() {
    let ticked = |id: &str| -> Vec<&str> { pack(id).unwrap().ticked().collect() };
    assert_eq!(
        ticked("creative"),
        ["blender", "krita", "gimp", "inkscape", "kdenlive", "obs-studio", "ffmpeg"]
    );
    let development = ticked("development");
    assert!(development.contains(&"vscodium") && !development.contains(&"vscode"));
    let all_development: Vec<_> = pack("development").unwrap().apps.iter().map(|(a, _)| *a).collect();
    for ai in ["comfypilot", "ollama", "alpaca", "vscode"] {
        assert!(all_development.contains(&ai), "{ai} belongs in Development");
    }
    assert_eq!(ticked("gaming"), ["steam", "heroic", "protonplus", "mangohud", "gamemode"]);
}

#[test]
fn auto_prefers_flatpak_on_stable_releases_and_native_on_fast_ones() {
    let (_u, ubuntu) = system(UBUNTU_2404, RTX_4070);
    assert_eq!(choose(&ubuntu, "gimp", Preference::Auto, None), Some(Source::Flatpak));
    assert_eq!(choose(&ubuntu, "ffmpeg", Preference::Auto, None), Some(Source::Native));
    // Ubuntu's firefox package only installs the Snap.
    assert_eq!(choose(&ubuntu, "firefox", Preference::Native, None), Some(Source::Flatpak));
    // Steam needs the host GPU stack: native even where Flatpak is the default.
    assert_eq!(choose(&ubuntu, "steam", Preference::Auto, None), Some(Source::Native));

    let (_a, arch) = system(ARCH, RTX_4070);
    assert_eq!(choose(&arch, "krita", Preference::Auto, None), Some(Source::Native));
    // A verified Flathub build beats an AUR build...
    assert_eq!(choose(&arch, "heroic", Preference::Auto, None), Some(Source::Flatpak));
    // ...unless the user prefers system packages.
    assert_eq!(choose(&arch, "heroic", Preference::Native, None), Some(Source::Native));
}

#[test]
fn per_app_policies_and_explicit_choices() {
    let (_a, arch) = system(ARCH, RTX_4070);
    // Editors run host toolchains: native even when Flatpak is preferred.
    assert_eq!(choose(&arch, "vscodium", Preference::Flatpak, None), Some(Source::Native));
    // An explicit per-app choice still wins.
    assert_eq!(choose(&arch, "vscodium", Preference::Auto, Some(Source::Flatpak)), Some(Source::Flatpak));
    // Bottles' developers support only the Flatpak.
    assert_eq!(choose(&arch, "bottles", Preference::Native, None), Some(Source::Flatpak));
    assert_eq!(choose(&arch, "bottles", Preference::Auto, Some(Source::Native)), Some(Source::Native));
    assert_eq!(choose(&arch, "gimp", Preference::Flatpak, None), Some(Source::Flatpak));
}

#[test]
fn gpu_aware_packages() {
    let ollama = |sys: &System| catalog(sys).app("ollama").unwrap().native.clone().unwrap().packages;
    let (_n, nvidia) = system(ARCH, RTX_4070);
    let (_r, amd) = system(ARCH, RX_7900);
    assert_eq!(ollama(&nvidia), ["ollama-cuda"]);
    assert_eq!(ollama(&amd), ["ollama-rocm"]);
    assert!(catalog(&nvidia).app("cuda").unwrap().available());
    assert!(!catalog(&amd).app("cuda").unwrap().available());
}

#[test]
fn comfypilot_installs_from_its_signed_release_everywhere() {
    for os in [ARCH, UBUNTU_2604, DEBIAN_13, FEDORA_44] {
        let (_f, sys) = system(os, RTX_4070);
        let catalog = catalog(&sys);
        let app = catalog.app("comfypilot").unwrap();
        assert!(matches!(app.native.as_ref().unwrap().via, Via::SignedRelease { .. }));
        let steps = app.steps(Source::Native).unwrap();
        assert!(matches!(&steps[0], Step::SignedPackage { package, .. } if package == "comfypilot"));
        assert!(app.icon_svg.is_some_and(|svg| svg.contains("<svg")));
    }
}

#[test]
fn comfypilot_is_an_appimage_where_qt_is_too_old_for_its_package() {
    const PASSWD: &str = "root:x:0:0::/root:/bin/bash\nburce:x:1000:1000::/home/burce:/bin/bash\n";
    for os in [UBUNTU_2404, MINT_22] {
        // Whose home it goes in: unknown without a passwd entry, so not offered.
        let (f, sys) = system(os, RTX_4070);
        assert!(!catalog(&sys).app("comfypilot").unwrap().available());

        f.file("etc/passwd", PASSWD);
        let sys = f.detect("KDE");
        let steps = catalog(&sys).app("comfypilot").unwrap().steps(Source::Native).unwrap();
        let Step::SignedAppImage { install, .. } = &steps[0] else { panic!("{steps:?}") };
        assert_eq!(install.path, std::path::Path::new("/home/burce/Applications/ComfyPilot.AppImage"));
        assert!(install.desktop.contains("Exec=\"/home/burce/Applications/ComfyPilot.AppImage\"\n"));
        assert!(install.desktop.contains("Icon=io.github.ArcticLatent.ComfyPilot\n"));
        assert_eq!(
            install.desktop_entry,
            std::path::Path::new(
                "/home/burce/.local/share/applications/io.github.ArcticLatent.ComfyPilot.desktop"
            )
        );
    }
}

#[test]
fn stacked_workflows_plan_on_arch() {
    let f = Fixture::new(ARCH);
    f.gpu("0000:01:00.0", 0x10de, 0x2786, true).kernel("7.2.11-arch1-1", "linux");
    let sys = f.detect("KDE");
    let catalog = catalog(&sys);
    let mut selection = Selection::from_packs(&catalog, &["creative", "development", "gaming"]).unwrap();
    selection.apps.push(AppPick { id: "comfypilot".into(), source: None });
    selection.tasks.push("debloat.kde".into());
    let plan = plan(&sys, &catalog, &selection).unwrap();

    let ids: Vec<&str> = plan.tasks.iter().map(|t| t.id.as_str()).collect();
    let first_app = ids.iter().position(|id| id.starts_with("app.")).unwrap();
    assert!(ids[..first_app].iter().all(|id| !id.starts_with("app.")), "system tasks run first");
    assert_eq!(ids.last(), Some(&"debloat.kde"), "removals run last");
    insta::assert_snapshot!("stacked_workflows_arch_plan", plan.render());
}

#[test]
fn selection_errors_are_specific() {
    let (_a, arch) = system(ARCH, RTX_4070);
    let catalog = catalog(&arch);
    let pick = |id: &str, source| Selection {
        apps: vec![AppPick { id: id.into(), source }],
        ..Selection::default()
    };
    assert_eq!(
        plan(&arch, &catalog, &pick("synfig", Some(Source::Native))),
        Err(PlanError::Unavailable { app: "Synfig Studio".into(), from: "the system's packages" })
    );
    assert_eq!(
        plan(&arch, &catalog, &pick("photoshop", None)),
        Err(PlanError::UnknownApp("photoshop".into()))
    );
    assert!(matches!(Selection::from_packs(&catalog, &["office"]), Err(PlanError::UnknownPack(_))));

    // On Ubuntu, GIMP comes from Flathub (Auto) and pulls in the Flathub task.
    let (_u, ubuntu) = system(UBUNTU_2404, RTX_4070);
    let plan = plan(&ubuntu, &bootcraft_core::catalog::catalog(&ubuntu), &pick("gimp", None)).unwrap();
    let ids: Vec<&str> = plan.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["flatpak.flathub", "app.gimp"]);
}
