//! Application-level Qt setup that cxx-qt-lib doesn't cover (`cpp/bootcraft_qt.h`).

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("bootcraft_qt.h");

        #[namespace = "bootcraft"]
        fn add_application_font(path: &QString) -> bool;
        #[namespace = "bootcraft"]
        fn set_application_font_family(family: &QString);
        #[namespace = "bootcraft"]
        fn set_window_icon(path: &QString);
        #[namespace = "bootcraft"]
        fn set_desktop_file_name(name: &QString);
    }
}

use cxx_qt_lib::QString;

/// The QML module's resources (`build.rs`).
const RESOURCES: &str = ":/qt/qml/Bootcraft/qml";
const FONTS: &[&str] =
    &["Geist-Regular", "Geist-Medium", "Geist-SemiBold", "Geist-Bold", "JetBrainsMono-Regular"];
/// The desktop file's name and the Wayland app id (packaging/linux).
pub const APP_ID: &str = "io.github.ArcticLatent.Bootcraft";

/// Call before the application object exists: Qt then registers the app id
/// with the desktop portal while it starts, before the portal theme's first
/// request would tie the D-Bus connection to an anonymous app (and Qt's later
/// registration failed with "Connection already associated with an
/// application ID"). Only with the desktop entry installed (a package): the
/// portal refuses names it can't find, as under `cargo run` or an AppImage.
pub fn set_app_id() {
    if desktop_entry_installed() {
        ffi::set_desktop_file_name(&QString::from(APP_ID));
    }
}

/// Whether `<APP_ID>.desktop` is in an XDG `applications` folder.
fn desktop_entry_installed() -> bool {
    let file = format!("applications/{APP_ID}.desktop");
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")));
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(dirs.split(':').filter(|d| !d.is_empty()).map(std::path::PathBuf::from))
        .any(|dir| dir.join(&file).is_file())
}

/// Call after the application object exists and before QML loads.
pub fn setup() {
    for font in FONTS {
        if !ffi::add_application_font(&QString::from(&format!("{RESOURCES}/fonts/{font}.ttf"))) {
            eprintln!("warning: couldn't load the bundled font {font}");
        }
    }
    ffi::set_application_font_family(&QString::from("Geist"));
    ffi::set_window_icon(&QString::from(&format!("{RESOURCES}/assets/bootcraft.svg")));
}
