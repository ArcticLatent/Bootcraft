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
/// (`data/polkit/io.github.arcticlatent.bootcraft.policy`).
const INSTALLED_HELPER: &str = "/usr/lib/bootcraft/bootcraft-helper";

/// `BOOTCRAFT_HELPER`, the installed helper, or the one built next to this
/// binary (development).
fn helper_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("BOOTCRAFT_HELPER") {
        return Ok(PathBuf::from(path));
    }
    if Path::new(INSTALLED_HELPER).exists() {
        return Ok(INSTALLED_HELPER.into());
    }
    let sibling = std::env::current_exe()
        .map_err(|e| format!("could not locate the app: {e}"))?
        .with_file_name("bootcraft-helper");
    if sibling.exists() { Ok(sibling) } else { Err("bootcraft-helper is not installed".into()) }
}

pub fn start(args: &Value) -> Result<Value, String> {
    let mut running = RUNNING.lock().map_err(|_| "internal error: lock poisoned")?;
    if running.is_some() {
        return Err("a setup is already running".into());
    }
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

    let helper = helper_path()?;
    let mut child = Command::new("pkexec")
        .arg(&helper)
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start pkexec: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("could not talk to the helper")?;
    let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    writeln!(stdin, "{line}").map_err(|e| format!("could not send the plan to the helper: {e}"))?;
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
                    Ok(event) => push_event("helper", event),
                    Err(_) => push_event("helper-stderr", json!({ "line": line })),
                }
            }
        }
        let code = child.wait().ok().and_then(|status| status.code());
        if let Ok(mut running) = RUNNING.lock() {
            *running = None;
        }
        // pkexec: 126 = the password prompt was dismissed, 127 = not authorized.
        let message = match code {
            Some(126) => Some("The password prompt was closed; nothing was changed."),
            Some(127) => Some("Authorization failed; nothing was changed."),
            _ => None,
        };
        push_event("helper-exit", json!({ "code": code, "message": message }));
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
