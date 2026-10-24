//! Executing plans: `apply` for terminals and `serve` for the GUI.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use bootcraft_core::catalog::catalog;
use bootcraft_core::detect::{Env, detect};
use bootcraft_core::exec::{Executor, SystemRunner, User};
use bootcraft_core::protocol::{Control, Event, Outcome, Request};
use bootcraft_core::{Plan, Selection, System, plan};

/// `bootcraft-helper apply`: run a plan from a terminal (under sudo).
pub fn apply(root: &Path, sys: &System, selection: &Selection, yes: bool) -> ExitCode {
    if !is_root() {
        eprintln!("error: apply changes the system and must run as root: sudo bootcraft-helper apply");
        return ExitCode::FAILURE;
    }
    let catalog = catalog(sys);
    let plan = match plan(sys, &catalog, selection) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    print!("{}", plan.render());
    if !yes && !confirm("\nProceed? [y/N] ") {
        println!("Nothing was changed.");
        return ExitCode::SUCCESS;
    }

    let Some(mut executor) = Executor::new(root, sys, SystemRunner, User::invoking(root)) else {
        eprintln!("error: {} is not supported", sys.os.pretty_name);
        return ExitCode::FAILURE;
    };
    let mut log = Log::create(root);
    let mut formatter = Formatter::default();
    let summary = executor.execute(&plan, &mut |event| {
        log.write(&event);
        // The plan was printed already; the formatter still needs its titles.
        let lines = formatter.lines(&event);
        if !matches!(event, Event::Plan { .. }) {
            for line in lines {
                println!("{line}");
            }
        }
    });
    let finished = Event::Finished { ok: summary.ok, reboot: summary.reboot, log: log.path_string() };
    log.write(&finished);
    for line in formatter.lines(&finished) {
        println!("{line}");
    }
    if summary.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// `bootcraft-helper serve`: the GUI's side channel, started through pkexec.
/// Reads one [`Request`] line, then accepts [`Control`] lines while it emits
/// [`Event`] lines.
pub fn serve(root: &Path) -> ExitCode {
    let send = |event: &Event| {
        let mut out = io::stdout().lock();
        let _ = serde_json::to_writer(&mut out, event);
        let _ = out.write_all(b"\n");
        let _ = out.flush();
    };
    let fail = |message: String| {
        send(&Event::Error { message });
        ExitCode::FAILURE
    };

    // Only the real system: a caller must not point root's edits elsewhere.
    if root != Path::new("/") {
        return fail("--root cannot be used with serve".into());
    }
    if !is_root() {
        return fail("the helper must run as root (the app starts it with pkexec)".into());
    }
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).is_err() {
        return fail("could not read the request".into());
    }
    let request: Request = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(err) => return fail(format!("invalid request: {err}")),
    };

    let mut env = Env::from_process();
    if request.desktop.is_some() {
        env.current_desktop.clone_from(&request.desktop);
    }
    if request.session.is_some() {
        env.session_type.clone_from(&request.session);
    }
    let mut sys = detect(root, &env);
    if let Some(choice) = request.nvidia {
        choice.apply(&mut sys);
    }
    let catalog = catalog(&sys);
    let selection = Selection { tasks: request.select, apps: request.apps, preference: request.preference };
    let plan: Plan = match plan(&sys, &catalog, &selection) {
        Ok(plan) => plan,
        Err(err) => return fail(err.to_string()),
    };
    let Some(mut executor) = Executor::new(root, &sys, SystemRunner, User::invoking(root)) else {
        return fail(format!("{} is not supported", sys.os.pretty_name));
    };

    let cancel = executor.cancel_flag();
    thread::spawn(move || {
        for line in io::stdin().lock().lines().map_while(Result::ok) {
            if matches!(serde_json::from_str::<Control>(&line), Ok(Control::Cancel)) {
                cancel.store(true, Ordering::SeqCst);
            }
        }
    });

    let mut log = Log::create(root);
    let summary = executor.execute(&plan, &mut |event| {
        log.write(&event);
        send(&event);
    });
    let finished = Event::Finished { ok: summary.ok, reboot: summary.reboot, log: log.path_string() };
    log.write(&finished);
    send(&finished);
    if summary.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn confirm(prompt: &str) -> bool {
    print!("{prompt}");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer).is_ok() && matches!(answer.trim(), "y" | "Y" | "yes")
}

/// Effective UID from /proc, to avoid an FFI call for one number.
fn is_root() -> bool {
    fs::read_to_string("/proc/self/status").is_ok_and(|status| {
        status.lines().find_map(|l| l.strip_prefix("Uid:")).and_then(|ids| ids.split_whitespace().nth(1))
            == Some("0")
    })
}

/// Human-readable lines for events, shared by the terminal and the log file.
#[derive(Default)]
struct Formatter {
    titles: HashMap<String, String>,
}

impl Formatter {
    fn lines(&mut self, event: &Event) -> Vec<String> {
        let title = |titles: &HashMap<String, String>, id: &str| {
            titles.get(id).cloned().unwrap_or_else(|| id.to_owned())
        };
        match event {
            Event::Plan { tasks } => {
                self.titles = tasks.iter().map(|t| (t.id.clone(), t.title.clone())).collect();
                std::iter::once(format!("Plan: {} tasks", tasks.len()))
                    .chain(tasks.iter().map(|t| format!("  - {} ({})", t.title, t.id)))
                    .collect()
            }
            Event::TaskStarted { id } => vec![format!("\n==> {}", title(&self.titles, id))],
            Event::Step { description, .. } => vec![format!("  -> {description}")],
            Event::Output { line } => vec![format!("     {line}")],
            Event::TaskFinished { id, outcome, message } => {
                let status = match outcome {
                    Outcome::Done => "done",
                    Outcome::AlreadyDone => "already done",
                    Outcome::Failed => "FAILED",
                    Outcome::Skipped => "skipped",
                    Outcome::Cancelled => "cancelled",
                };
                let detail = message.as_deref().map(|m| format!(": {m}")).unwrap_or_default();
                vec![format!("  [{status}] {}{detail}", title(&self.titles, id))]
            }
            Event::Finished { ok, reboot, log } => {
                let mut lines =
                    vec![if *ok { "\nAll done.".to_owned() } else { "\nFinished with problems.".to_owned() }];
                if *reboot {
                    lines.push("Reboot to finish the setup.".into());
                }
                if let Some(log) = log {
                    lines.push(format!("Log: {log}"));
                }
                lines
            }
            Event::Error { message } => vec![format!("error: {message}")],
        }
    }
}

/// `/var/log/bootcraft/bootcraft-<UTC time>.log`. Logging is best effort: a
/// read-only /var/log must not stop the setup.
struct Log {
    file: Option<File>,
    path: PathBuf,
    formatter: Formatter,
}

impl Log {
    fn create(root: &Path) -> Self {
        let dir = root.join("var/log/bootcraft");
        let path = dir.join(format!("bootcraft-{}.log", utc_timestamp()));
        let file = fs::create_dir_all(&dir).and_then(|()| File::create(&path)).ok();
        Self { file, path, formatter: Formatter::default() }
    }

    fn path_string(&self) -> Option<String> {
        self.file.as_ref().map(|_| self.path.display().to_string())
    }

    fn write(&mut self, event: &Event) {
        let Some(file) = &mut self.file else { return };
        for line in self.formatter.lines(event) {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn utc_timestamp() -> String {
    format_utc(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

/// `YYYYMMDD-HHMMSS` in UTC.
fn format_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days.cast_signed() + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::format_utc;

    #[test]
    fn utc_formatting() {
        assert_eq!(format_utc(0), "19700101-000000");
        assert_eq!(format_utc(951_782_400), "20000229-000000");
        assert_eq!(format_utc(1_761_264_123), "20251024-000203");
        assert_eq!(format_utc(4_102_444_799), "20991231-235959");
    }
}
