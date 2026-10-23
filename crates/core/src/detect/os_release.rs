use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::system::OsInfo;

/// Read `/etc/os-release`, falling back to `/usr/lib/os-release` as the spec
/// requires. A missing file yields an empty, unsupported [`OsInfo`].
pub fn read_os_release(root: &Path) -> OsInfo {
    ["etc/os-release", "usr/lib/os-release"]
        .iter()
        .find_map(|path| fs::read_to_string(root.join(path)).ok())
        .map(|text| parse_os_release(&text))
        .unwrap_or_else(|| parse_os_release(""))
}

pub fn parse_os_release(text: &str) -> OsInfo {
    let fields: HashMap<&str, String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), unquote(value.trim())))
        .collect();
    let get = |key: &str| fields.get(key).cloned().filter(|v| !v.is_empty());

    let id = get("ID").unwrap_or_else(|| "linux".into()).to_ascii_lowercase();
    OsInfo {
        id_like: get("ID_LIKE")
            .map(|v| v.split_whitespace().map(str::to_ascii_lowercase).collect())
            .unwrap_or_default(),
        version_id: get("VERSION_ID"),
        pretty_name: get("PRETTY_NAME").or_else(|| get("NAME")).unwrap_or_else(|| id.clone()),
        id,
    }
}

/// Strip shell-style quotes and the backslash escapes os-release allows.
fn unquote(value: &str) -> String {
    let inner = match value.as_bytes() {
        [b'"', .., b'"'] | [b'\'', .., b'\''] if value.len() >= 2 => &value[1..value.len() - 1],
        _ => value,
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::Distro;

    #[test]
    fn parses_quoted_and_unquoted_fields() {
        let os = parse_os_release(
            "# comment\nNAME=\"Linux Mint\"\nVERSION_ID=\"22.2\"\nID=linuxmint\nID_LIKE=\"ubuntu debian\"\nPRETTY_NAME='Linux Mint 22.2'\n",
        );
        assert_eq!(os.id, "linuxmint");
        assert_eq!(os.id_like, ["ubuntu", "debian"]);
        assert_eq!(os.version_id.as_deref(), Some("22.2"));
        assert_eq!(os.pretty_name, "Linux Mint 22.2");
        assert_eq!(os.distro(), Some(Distro::Mint));
    }

    #[test]
    fn derivatives_are_not_their_parent() {
        // Ubuntu lists ID_LIKE=debian; it must never be treated as Debian.
        let ubuntu = parse_os_release("ID=ubuntu\nID_LIKE=debian\nVERSION_ID=\"24.04\"\n");
        assert_eq!(ubuntu.distro(), Some(Distro::Ubuntu));
        let manjaro = parse_os_release("ID=manjaro\nID_LIKE=arch\n");
        assert_eq!(manjaro.distro(), None);
    }

    #[test]
    fn version_support() {
        let supported = |text: &str| parse_os_release(text).version_supported();
        assert!(supported("ID=fedora\nVERSION_ID=44\n"));
        assert!(!supported("ID=fedora\nVERSION_ID=42\n"));
        assert!(supported("ID=ubuntu\nVERSION_ID=\"26.04\"\n"));
        assert!(!supported("ID=ubuntu\nVERSION_ID=\"22.04\"\n"));
        assert!(!supported("ID=ubuntu\nVERSION_ID=\"25.10\"\n"));
        assert!(supported("ID=debian\nVERSION_ID=\"13\"\n"));
        assert!(!supported("ID=debian\nVERSION_ID=\"12\"\n"));
        assert!(supported("ID=arch\n"));
    }

    #[test]
    fn unquote_handles_escapes() {
        assert_eq!(unquote(r#""a \"b\" c""#), r#"a "b" c"#);
        assert_eq!(unquote("plain"), "plain");
    }
}
