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
pub const APP_ID: &str = "io.github.arcticlatent.Bootcraft";

/// Call after the application object exists and before QML loads.
pub fn setup() {
    for font in FONTS {
        if !ffi::add_application_font(&QString::from(&format!("{RESOURCES}/fonts/{font}.ttf"))) {
            eprintln!("warning: couldn't load the bundled font {font}");
        }
    }
    ffi::set_application_font_family(&QString::from("Geist"));
    ffi::set_window_icon(&QString::from(&format!("{RESOURCES}/assets/bootcraft.svg")));
    ffi::set_desktop_file_name(&QString::from(APP_ID));
}
