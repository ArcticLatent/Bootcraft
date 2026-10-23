//! A fake root filesystem for detection, probe and plan tests.

use std::fs;
use std::path::Path;

use bootcraft_core::System;
use bootcraft_core::detect::{Env, detect};
use tempfile::TempDir;

pub const ARCH: &str = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n";

pub const PACMAN_CONF: &str = "\
[options]
HoldPkg     = pacman glibc
Architecture = auto
#Color
CheckSpace
ParallelDownloads = 5

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

#[multilib]
#Include = /etc/pacman.d/mirrorlist
";

pub struct Fixture {
    pub dir: TempDir,
}

impl Fixture {
    pub fn new(os_release: &str) -> Self {
        let fixture = Self { dir: TempDir::new().unwrap() };
        fixture.file("etc/os-release", os_release);
        fixture.file("proc/sys/kernel/osrelease", "7.2.11-arch1-1\n");
        fixture
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn file(&self, path: &str, contents: &str) -> &Self {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }

    pub fn gpu(&self, slot: &str, vendor: u16, device: u16, boot_vga: bool) -> &Self {
        let dir = format!("sys/bus/pci/devices/{slot}");
        self.file(&format!("{dir}/class"), "0x030000\n");
        self.file(&format!("{dir}/vendor"), &format!("0x{vendor:04x}\n"));
        self.file(&format!("{dir}/device"), &format!("0x{device:04x}\n"));
        self.file(&format!("{dir}/boot_vga"), if boot_vga { "1\n" } else { "0\n" })
    }

    pub fn kernel(&self, version: &str, pkgbase: &str) -> &Self {
        self.file(&format!("usr/lib/modules/{version}/pkgbase"), &format!("{pkgbase}\n"))
    }

    /// An installed pacman package, with optional provides.
    pub fn package(&self, name: &str, provides: &[&str]) -> &Self {
        let mut desc = format!("%NAME%\n{name}\n\n%VERSION%\n1.0-1\n\n");
        if !provides.is_empty() {
            desc.push_str(&format!("%PROVIDES%\n{}\n\n", provides.join("\n")));
        }
        self.file(&format!("var/lib/pacman/local/{name}-1.0-1/desc"), &desc)
    }

    pub fn secure_boot(&self, enabled: bool) -> &Self {
        let path =
            self.root().join("sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, [6, 0, 0, 0, u8::from(enabled)]).unwrap();
        self
    }

    pub fn detect(&self, desktop: &str) -> System {
        let env = Env { current_desktop: Some(desktop.into()), session_type: Some("wayland".into()) };
        detect(self.root(), &env)
    }
}
