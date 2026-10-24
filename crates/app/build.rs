use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

/// QML files of the `Bootcraft` module, relative to this crate.
const QML_FILES: &[&str] = &[
    "qml/Main.qml",
    "qml/Icon.qml",
    "qml/AppIcon.qml",
    "qml/LogoMark.qml",
    "qml/Sidebar.qml",
    "qml/NavItem.qml",
    "qml/SystemStrip.qml",
    "qml/WorkflowCard.qml",
    "qml/CheckMark.qml",
    "qml/Pill.qml",
    "qml/SourceBadge.qml",
    "qml/ItemRow.qml",
    "qml/PlanTab.qml",
    "qml/AppTile.qml",
    "qml/ReadyCard.qml",
    "qml/PrimaryButton.qml",
    "qml/LinkButton.qml",
    "qml/Segmented.qml",
    "qml/PageHeader.qml",
    "qml/LogView.qml",
    "qml/TerminalDrawer.qml",
    "qml/OverviewPage.qml",
    "qml/ApplicationsPage.qml",
    "qml/SystemPage.qml",
    "qml/ActivityPage.qml",
    "qml/SettingsPage.qml",
];
const SINGLETONS: &[&str] = &["qml/Theme.qml", "qml/Api.qml", "qml/Setup.qml"];

fn main() {
    println!("cargo::rerun-if-changed=cpp");
    let mut qml: Vec<QmlFile> = QML_FILES.iter().map(|path| QmlFile::from(*path)).collect();
    qml.extend(SINGLETONS.iter().map(|path| QmlFile::from(*path).singleton(true)));

    // Icons, logos and fonts, served as qrc:/qt/qml/Bootcraft/qml/<dir>/<file>.
    let mut assets = Vec::new();
    for dir in ["qml/icons", "qml/logos", "qml/fonts", "qml/assets"] {
        println!("cargo::rerun-if-changed={dir}");
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{dir}: {e}"))
            .filter_map(Result::ok)
            .map(|entry| format!("{dir}/{}", entry.file_name().to_string_lossy()))
            .filter(|path| [".svg", ".ttf"].iter().any(|ext| path.ends_with(ext)))
            .collect();
        files.sort();
        assets.extend(files);
    }

    CxxQtBuilder::new_qml_module(QmlModule::new("Bootcraft").qml_files(qml))
        .qrc_resources(assets.iter().map(String::as_str))
        .include_dir("cpp")
        .files(["src/bridge.rs", "src/qt_app.rs"])
        .build();

    // ld.bfd with --as-needed (Fedora's and Arch's Rust; rustup's uses lld)
    // keeps a shared library only if something linked before it uses it, and
    // cxx-qt lists Qt before its static library that calls Qt. List them again
    // after it. (The same fix as ComfyPilot's.)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        for lib in ["Qt6QuickControls2", "Qt6Quick", "Qt6Qml", "Qt6Gui", "Qt6Core"] {
            println!("cargo::rustc-link-lib=dylib={lib}");
        }
    }
}
