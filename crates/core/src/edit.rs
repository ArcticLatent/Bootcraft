//! Structured config-file edits.
//!
//! Each edit is a pure `&str -> String` transform, so it can be unit tested,
//! previewed and checked for "already applied" (applying it changes nothing).

use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Edit {
    /// pacman.conf-style bare flag (`Color`, `BottomUp`): uncomment it inside
    /// `[section]`, or add it at the end of that section.
    EnableFlag { section: String, flag: String },
    /// pacman.conf: activate a repository that ships commented out, such as
    /// `[multilib]`, together with its `Include`/`Server` lines.
    EnablePacmanRepo { repo: String },
    /// INI-style `key=value` inside `[section]`: replace an active or commented
    /// assignment, or add one. The section is created if missing.
    SetKey { section: String, key: String, value: String },
}

impl Edit {
    pub fn flag(section: &str, flag: &str) -> Self {
        Self::EnableFlag { section: section.into(), flag: flag.into() }
    }

    pub fn pacman_repo(repo: &str) -> Self {
        Self::EnablePacmanRepo { repo: repo.into() }
    }

    pub fn set_key(section: &str, key: &str, value: &str) -> Self {
        Self::SetKey { section: section.into(), key: key.into(), value: value.into() }
    }

    pub fn apply(&self, text: &str) -> String {
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        match self {
            Self::EnableFlag { section, flag } => enable_flag(&mut lines, section, flag),
            Self::EnablePacmanRepo { repo } => enable_pacman_repo(&mut lines, repo),
            Self::SetKey { section, key, value } => set_key(&mut lines, section, key, value),
        }
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }

    /// True when the file already has the change.
    pub fn is_applied(&self, text: &str) -> bool {
        let normalized = if text.ends_with('\n') { text.to_owned() } else { format!("{text}\n") };
        self.apply(text) == normalized
    }
}

impl fmt::Display for Edit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnableFlag { section, flag } => write!(f, "enable {flag} in [{section}]"),
            Self::EnablePacmanRepo { repo } => write!(f, "enable the [{repo}] repository"),
            Self::SetKey { section, key, value } => write!(f, "set {key}={value} in [{section}]"),
        }
    }
}

fn header_name(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

/// The text of a commented-out line (`# Color` -> `Color`), or `None`.
fn uncommented(line: &str) -> Option<&str> {
    Some(line.trim_start().strip_prefix('#')?.trim_start())
}

/// Line range `(header, end)` of a section, where `end` is the next header or EOF.
fn section_range(lines: &[String], section: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|l| header_name(l) == Some(section))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| header_name(l).is_some())
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some((start, end))
}

/// Insert after the last non-blank line of a section, keeping its trailing blank lines.
fn insert_at_section_end(lines: &mut Vec<String>, start: usize, end: usize, line: String) {
    let last = (start..end).rev().find(|&i| !lines[i].trim().is_empty()).unwrap_or(start);
    lines.insert(last + 1, line);
}

fn enable_flag(lines: &mut Vec<String>, section: &str, flag: &str) {
    let is_flag = |text: &str| text.split_whitespace().next() == Some(flag);
    let Some((start, end)) = section_range(lines, section) else {
        lines.push(String::new());
        lines.push(format!("[{section}]"));
        lines.push(flag.to_owned());
        return;
    };
    if lines[start + 1..end].iter().any(|l| is_flag(l.trim())) {
        return;
    }
    if let Some(i) = (start + 1..end).find(|&i| uncommented(&lines[i]).is_some_and(is_flag)) {
        lines[i] = uncommented(&lines[i]).unwrap_or_default().to_owned();
        return;
    }
    insert_at_section_end(lines, start, end, flag.to_owned());
}

fn enable_pacman_repo(lines: &mut Vec<String>, repo: &str) {
    if lines.iter().any(|l| header_name(l) == Some(repo)) {
        return;
    }
    let commented = lines.iter().position(|l| uncommented(l).and_then(header_name) == Some(repo));
    let Some(header) = commented else {
        lines.push(String::new());
        lines.push(format!("[{repo}]"));
        lines.push("Include = /etc/pacman.d/mirrorlist".to_owned());
        return;
    };
    lines[header] = format!("[{repo}]");
    // Uncomment the repo's own settings, which follow the header until a blank line.
    for line in lines.iter_mut().skip(header + 1) {
        match uncommented(line) {
            Some(text) if text.contains('=') => *line = text.to_owned(),
            _ => break,
        }
    }
}

fn set_key(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let assignment = format!("{key}={value}");
    let is_key = |text: &str| text.split_once('=').is_some_and(|(k, _)| k.trim() == key);
    let Some((start, end)) = section_range(lines, section) else {
        lines.push(String::new());
        lines.push(format!("[{section}]"));
        lines.push(assignment);
        return;
    };
    if let Some(i) = (start + 1..end).find(|&i| is_key(lines[i].trim())) {
        lines[i] = assignment;
    } else if let Some(i) = (start + 1..end).find(|&i| uncommented(&lines[i]).is_some_and(is_key)) {
        lines[i] = assignment;
    } else {
        insert_at_section_end(lines, start, end, assignment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACMAN_CONF: &str = "\
[options]
HoldPkg     = pacman glibc
Architecture = auto

# Misc options
#UseSyslog
#Color
#NoProgressBar
CheckSpace
#VerbosePkgLists
ParallelDownloads = 5

[core]
Include = /etc/pacman.d/mirrorlist

#[multilib-testing]
#Include = /etc/pacman.d/mirrorlist

#[multilib]
#Include = /etc/pacman.d/mirrorlist
";

    #[test]
    fn uncomments_existing_flag() {
        let out = Edit::flag("options", "Color").apply(PACMAN_CONF);
        assert!(out.contains("\nColor\n"));
        assert!(!out.contains("#Color"));
        assert!(Edit::flag("options", "Color").is_applied(&out));
    }

    #[test]
    fn adds_missing_flag_inside_section() {
        let out = Edit::flag("options", "ILoveCandy").apply(PACMAN_CONF);
        assert!(out.contains("ParallelDownloads = 5\nILoveCandy\n\n[core]"));
        assert!(Edit::flag("options", "ILoveCandy").is_applied(&out));
    }

    #[test]
    fn flag_matching_is_exact() {
        // `CheckSpace` must not satisfy a flag named `Check`.
        let out = Edit::flag("options", "Check").apply(PACMAN_CONF);
        assert!(out.contains("ParallelDownloads = 5\nCheck\n"));
    }

    #[test]
    fn enables_multilib_but_not_multilib_testing() {
        let edit = Edit::pacman_repo("multilib");
        let out = edit.apply(PACMAN_CONF);
        assert!(out.contains("#[multilib-testing]\n#Include = /etc/pacman.d/mirrorlist"));
        assert!(out.ends_with("[multilib]\nInclude = /etc/pacman.d/mirrorlist\n"));
        assert!(edit.is_applied(&out));
        assert!(!edit.is_applied(PACMAN_CONF));
    }

    #[test]
    fn appends_repo_when_absent() {
        let out = Edit::pacman_repo("multilib").apply("[options]\n");
        assert_eq!(out, "[options]\n\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n");
    }

    #[test]
    fn set_key_replaces_commented_value_in_the_right_section() {
        let conf = "[LightDM]\n#greeter-session=other\n\n[Seat:*]\n#type=local\n#greeter-session=example-gtk-gnome\n#user-session=default\n";
        let edit = Edit::set_key("Seat:*", "greeter-session", "lightdm-slick-greeter");
        let out = edit.apply(conf);
        assert_eq!(
            out,
            "[LightDM]\n#greeter-session=other\n\n[Seat:*]\n#type=local\ngreeter-session=lightdm-slick-greeter\n#user-session=default\n"
        );
        assert!(edit.is_applied(&out));
    }

    #[test]
    fn set_key_creates_section() {
        let out = Edit::set_key("Seat:*", "greeter-session", "x").apply("[LightDM]\n");
        assert_eq!(out, "[LightDM]\n\n[Seat:*]\ngreeter-session=x\n");
    }
}
