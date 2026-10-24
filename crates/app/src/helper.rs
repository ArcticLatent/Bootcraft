//! Running the setup: `pkexec bootcraft-helper serve`, one per run.
//!
//! The app sends one request line on the helper's stdin and relays every
//! event line from its stdout to QML as a `helper` event. A cancel request is
//! one more line on stdin. When the helper exits, a `helper-exit` event says
//! how it ended, including pkexec's codes for a dismissed or failed password
//! prompt.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Mutex;

use bootcraft_core::detect::Env;
use bootcraft_core::protocol::{Control, Request};
use serde_json::{Value, json};

use crate::bridge::push_event;

/// The running helper's stdin, while a run is in progress.
static RUNNING: Mutex<Option<ChildStdin>> = Mutex::new(None);

/// Where the helper is installed for the polkit policy
/// (`packaging/linux/io.github.ArcticLatent.Bootcraft.policy`).
pub const INSTALLED_HELPER: &str = "/usr/lib/bootcraft/bootcraft-helper";

/// `BOOTCRAFT_HELPER`, the AppImage's own helper, the installed helper, or
/// the one built next to this binary (development).
fn helper_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("BOOTCRAFT_HELPER") {
        return Ok(PathBuf::from(path));
    }
    if let Some(appdir) = std::env::var_os("APPDIR") {
        return appimage_helper(&PathBuf::from(appdir).join("usr/lib/bootcraft/bootcraft-helper"));
    }
    if Path::new(INSTALLED_HELPER).exists() {
        return Ok(INSTALLED_HELPER.into());
    }
    let sibling = std::env::current_exe()
        .map_err(|e| format!("could not locate the app: {e}"))?
        .with_file_name("bootcraft-helper");
    if sibling.exists() { Ok(sibling) } else { Err("bootcraft-helper is not installed".into()) }
}

/// An AppImage's files live on a FUSE mount that root can't read, so pkexec
/// couldn't start the helper from there. Copy it (a static binary) into the
/// user's runtime directory first.
fn appimage_helper(bundled: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let runtime =
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).ok_or("XDG_RUNTIME_DIR is not set")?;
    let copy = runtime.join("bootcraft-helper");
    std::fs::copy(bundled, &copy).map_err(|e| format!("could not prepare the helper: {e}"))?;
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("could not prepare the helper: {e}"))?;
    Ok(copy)
}

pub fn start(args: &Value) -> Result<Value, String> {
    // pkexec drops the desktop variables, so the app passes its own.
    let env = Env::from_process();
    let request = Request {
        select: serde_json::from_value(args["tasks"].clone()).unwrap_or_default(),
        apps: serde_json::from_value(args["apps"].clone()).unwrap_or_default(),
        preference: serde_json::from_value(args["preference"].clone()).unwrap_or_default(),
        desktop: env.current_desktop,
        session: env.session_type,
        nvidia: serde_json::from_value(args["nvidia"].clone()).ok(),
    };
    let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    spawn("serve", Some(&line), "helper", |code| {
        // pkexec: 126 = the password prompt was dismissed, 127 = not authorized.
        let message = match code {
            Some(126) => Some("The password prompt was closed; nothing was changed."),
            Some(127) => Some("Authorization failed; nothing was changed."),
            _ => None,
        };
        push_event("helper-exit", json!({ "code": code, "message": message }));
    })
}

/// Install a newer Bootcraft package: `bootcraft-helper self-update`, which
/// checks the signed release itself. Its events arrive as `update-event`;
/// `update-exit` says how it ended, and on success the app starts again
/// once it quits.
pub fn start_self_update() -> Result<Value, String> {
    spawn("self-update", None, "update-event", |code| {
        let message = match code {
            Some(0) => None,
            Some(126) => Some("The password prompt was closed; nothing was changed.".to_owned()),
            Some(127) => Some("Authorization failed; nothing was changed.".to_owned()),
            _ => Some("The update was not installed.".to_owned()),
        };
        let restart = code == Some(0) && crate::update::relaunch_after_exit().is_ok();
        push_event("update-exit", json!({ "ok": code == Some(0), "restart": restart, "message": message }));
    })
}

/// `pkexec bootcraft-helper <command>`, with `request` as its first stdin
/// line. Its JSON event lines go to QML as `event`, other output as
/// `helper-stderr`; `on_exit` gets its exit code.
fn spawn(
    command: &str,
    request: Option<&str>,
    event: &'static str,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<Value, String> {
    let mut running = RUNNING.lock().map_err(|_| "internal error: lock poisoned")?;
    if running.is_some() {
        return Err("a setup is already running".into());
    }
    let helper = helper_path()?;
    let mut child = Command::new("pkexec")
        .arg(&helper)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start pkexec: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("could not talk to the helper")?;
    if let Some(line) = request {
        writeln!(stdin, "{line}").map_err(|e| format!("could not send the plan to the helper: {e}"))?;
    }
    *running = Some(stdin);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    push_event("helper-stderr", json!({ "line": line }));
                }
            });
        }
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                match serde_json::from_str::<Value>(&line) {
                    Ok(payload) => push_event(event, payload),
                    Err(_) => push_event("helper-stderr", json!({ "line": line })),
                }
            }
        }
        let code = child.wait().ok().and_then(|status| status.code());
        if let Ok(mut running) = RUNNING.lock() {
            *running = None;
        }
        on_exit(code);
    });
    Ok(json!({ "helper": helper }))
}

/// Ask the helper to stop after the step that is running.
pub fn cancel() -> Result<Value, String> {
    let mut running = RUNNING.lock().map_err(|_| "internal error: lock poisoned")?;
    let stdin = running.as_mut().ok_or("nothing is running")?;
    let line = serde_json::to_string(&Control::Cancel).map_err(|e| e.to_string())?;
    writeln!(stdin, "{line}").map_err(|e| format!("could not reach the helper: {e}"))?;
    Ok(Value::Null)
}
