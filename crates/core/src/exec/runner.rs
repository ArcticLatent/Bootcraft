//! Running external programs, behind a trait so the executor can be tested
//! without touching the system.

use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

/// A regular account that AUR packages are built as; makepkg refuses root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub name: String,
    pub uid: u32,
    pub home: PathBuf,
}

impl User {
    /// Look up `uid` in `<root>/etc/passwd`.
    pub fn from_uid(root: &Path, uid: u32) -> Option<Self> {
        let passwd = fs::read_to_string(root.join("etc/passwd")).ok()?;
        passwd.lines().find_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            (fields.len() >= 6 && fields[2].parse() == Ok(uid)).then(|| Self {
                name: fields[0].to_owned(),
                uid,
                home: fields[5].into(),
            })
        })
    }

    /// The account that started Bootcraft through pkexec or sudo. `None` when
    /// root started it directly (e.g. `su -c`).
    pub fn invoking(root: &Path) -> Option<Self> {
        let uid =
            ["PKEXEC_UID", "SUDO_UID"].iter().find_map(|var| std::env::var(var).ok()?.parse::<u32>().ok())?;
        if uid == 0 { None } else { Self::from_uid(root, uid) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub argv: Vec<String>,
    /// Run as this user instead of root.
    pub user: Option<User>,
    pub cwd: Option<PathBuf>,
}

impl Cmd {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { argv: argv.into_iter().map(Into::into).collect(), user: None, cwd: None }
    }

    #[must_use]
    pub fn as_user(mut self, user: &User) -> Self {
        self.user = Some(user.clone());
        self
    }

    #[must_use]
    pub fn in_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// `$ argv` as shown in the log, noting the user when it isn't root.
    pub fn display(&self) -> String {
        let line = format!("$ {}", self.argv.join(" "));
        match &self.user {
            Some(user) => format!("{line}  (as {})", user.name),
            None => line,
        }
    }
}

pub trait Runner {
    /// Run to completion, passing each line of stdout and stderr to `output`.
    /// Returns whether the program exited successfully.
    fn run(&mut self, cmd: &Cmd, output: &mut dyn FnMut(&str)) -> io::Result<bool>;

    /// Run and capture stdout, for queries.
    fn capture(&mut self, cmd: &Cmd) -> io::Result<(bool, String)>;
}

/// Runs real processes. Stdin is closed so a program that unexpectedly
/// prompts fails instead of hanging.
pub struct SystemRunner;

impl SystemRunner {
    fn command(cmd: &Cmd) -> Command {
        let mut command = match &cmd.user {
            Some(user) => {
                // runuser keeps root's environment; give the user a clean one.
                let mut command = Command::new("runuser");
                command
                    .args(["-u", &user.name, "--", "env", "-i"])
                    .arg(format!("HOME={}", user.home.display()))
                    .arg(format!("USER={}", user.name))
                    .arg(format!("LOGNAME={}", user.name))
                    .arg("PATH=/usr/local/bin:/usr/bin:/bin");
                if let Ok(lang) = std::env::var("LANG") {
                    command.arg(format!("LANG={lang}"));
                }
                command.args(&cmd.argv);
                command
            }
            None => {
                let mut command = Command::new(&cmd.argv[0]);
                command.args(&cmd.argv[1..]);
                command
            }
        };
        if let Some(dir) = &cmd.cwd {
            command.current_dir(dir);
        }
        command.stdin(Stdio::null());
        command
    }
}

impl Runner for SystemRunner {
    fn run(&mut self, cmd: &Cmd, output: &mut dyn FnMut(&str)) -> io::Result<bool> {
        let mut child = Self::command(cmd).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        let streams: [Option<Box<dyn Read + Send>>; 2] = [
            child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
            child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
        ];
        let readers: Vec<_> = streams
            .into_iter()
            .flatten()
            .map(|stream| {
                let tx = tx.clone();
                thread::spawn(move || forward_lines(stream, &tx))
            })
            .collect();
        drop(tx);
        for line in rx {
            output(&line);
        }
        for reader in readers {
            let _ = reader.join();
        }
        Ok(child.wait()?.success())
    }

    fn capture(&mut self, cmd: &Cmd) -> io::Result<(bool, String)> {
        let out = Self::command(cmd).stderr(Stdio::null()).output()?;
        Ok((out.status.success(), String::from_utf8_lossy(&out.stdout).into_owned()))
    }
}

fn forward_lines(stream: impl Read, tx: &mpsc::Sender<String>) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
        let text = String::from_utf8_lossy(&buf);
        // Progress bars redraw with \r; keep the final state of the line.
        let line = text.trim_end_matches(['\n', '\r']).rsplit('\r').next().unwrap_or_default();
        if tx.send(line.to_owned()).is_err() {
            break;
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_stdout_and_stderr_and_reports_status() {
        let cmd = Cmd::new(["sh", "-c", "echo out; echo err >&2; printf '10%%\\r50%%\\r100%%\\n'; exit 3"]);
        let mut lines = Vec::new();
        let ok = SystemRunner.run(&cmd, &mut |l| lines.push(l.to_owned())).unwrap();
        assert!(!ok);
        lines.sort();
        assert_eq!(lines, ["100%", "err", "out"]);
    }

    #[test]
    fn stdin_is_closed_so_prompts_cannot_hang() {
        let cmd = Cmd::new(["sh", "-c", "read answer || echo eof"]);
        let (ok, out) = SystemRunner.capture(&cmd).unwrap();
        assert!(ok);
        assert_eq!(out, "eof\n");
    }

    #[test]
    fn missing_program_is_an_error() {
        assert!(SystemRunner.run(&Cmd::new(["bootcraft-no-such-program"]), &mut |_| {}).is_err());
    }

    #[test]
    fn user_lookup_from_passwd() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("etc")).unwrap();
        fs::write(
            dir.path().join("etc/passwd"),
            "root:x:0:0::/root:/bin/bash\nburce:x:1000:1000:Burce:/home/burce:/bin/zsh\n",
        )
        .unwrap();
        let user = User::from_uid(dir.path(), 1000).unwrap();
        assert_eq!((user.name.as_str(), user.home.as_path()), ("burce", Path::new("/home/burce")));
        assert_eq!(User::from_uid(dir.path(), 1001), None);
    }
}
