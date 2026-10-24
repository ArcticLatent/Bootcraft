//! Bootcraft's desktop app: the Qt Quick interface over `bootcraft-core`.
//!
//! The app itself never changes the system. It detects, shows the catalog and
//! the plan, and starts `bootcraft-helper serve` through pkexec to apply it
//! (see `helper.rs`).

mod bridge;
mod commands;
mod helper;
mod icons;
mod qt_app;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

const MAIN_QML: &str = "qrc:/qt/qml/Bootcraft/qml/Main.qml";

fn main() {
    // The look comes from Bootcraft's own components; keep the platform style out of the way.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: no other thread exists yet, so nothing reads the environment concurrently.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Basic") };
    }
    // File dialogs from the desktop itself (Plasma's on KDE, GNOME's file
    // chooser on GNOME...) through the XDG desktop portal. Desktop Qt themes
    // only give native dialogs to widget apps; without a portal Qt falls back
    // to its own dialog, as it would anyway.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        // SAFETY: as above.
        unsafe { std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal") };
    }

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Bootcraft"));
        app.as_mut().set_organization_name(&QString::from("Arctic Latent"));
        app.as_mut().set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }
    qt_app::setup();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // Without a window nothing would ever quit the event loop.
        engine
            .as_mut()
            .on_object_creation_failed(|_, url| {
                eprintln!("error: the interface failed to load ({url}); see the QML messages above");
                std::process::exit(1);
            })
            .release();
        engine.load(&QUrl::from(MAIN_QML));
    }

    let code = app.as_mut().map_or(1, QGuiApplication::exec);
    drop(engine);
    std::process::exit(code);
}
