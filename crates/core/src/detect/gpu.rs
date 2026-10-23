use std::fs;
use std::path::Path;

use crate::system::{Gpu, GpuVendor};

/// List display controllers (PCI class 0x03xxxx) from sysfs, so detection
/// works on minimal installs without `pciutils`.
pub fn scan_gpus(root: &Path) -> Vec<Gpu> {
    let Ok(entries) = fs::read_dir(root.join("sys/bus/pci/devices")) else {
        return Vec::new();
    };
    let pci_ids = ["usr/share/hwdata/pci.ids", "usr/share/misc/pci.ids"]
        .iter()
        .find_map(|path| fs::read_to_string(root.join(path)).ok());

    let mut gpus: Vec<Gpu> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let hex = |name: &str| {
                let text = fs::read_to_string(dir.join(name)).ok()?;
                u32::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
            };
            if hex("class")? >> 16 != 0x03 {
                return None;
            }
            let vendor_id = u16::try_from(hex("vendor")?).ok()?;
            let device_id = u16::try_from(hex("device")?).ok()?;
            Some(Gpu {
                slot: entry.file_name().to_string_lossy().into_owned(),
                vendor: GpuVendor::from_pci_id(vendor_id),
                vendor_id,
                device_id,
                name: pci_ids.as_deref().and_then(|db| lookup_pci_name(db, vendor_id, device_id)),
                boot_vga: fs::read_to_string(dir.join("boot_vga")).is_ok_and(|v| v.trim() == "1"),
            })
        })
        .collect();
    gpus.sort_by(|a, b| a.slot.cmp(&b.slot));
    gpus
}

/// Find a device name in the pci.ids database. Vendor lines start at column
/// 0, their devices follow indented by one tab.
pub fn lookup_pci_name(db: &str, vendor_id: u16, device_id: u16) -> Option<String> {
    let vendor = format!("{vendor_id:04x}  ");
    let device = format!("\t{device_id:04x}  ");
    let mut in_vendor = false;
    for line in db.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            if in_vendor {
                return None;
            }
            in_vendor = line.starts_with(&vendor);
        } else if in_vendor && let Some(name) = line.strip_prefix(&device) {
            return Some(name.trim().to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::NvidiaGen;

    #[test]
    fn nvidia_generations() {
        let cases = [
            (0x2b85, NvidiaGen::Modern),      // RTX 5090
            (0x2684, NvidiaGen::Modern),      // RTX 4090
            (0x2204, NvidiaGen::Modern),      // RTX 3090
            (0x1e07, NvidiaGen::Modern),      // RTX 2080 Ti
            (0x1f82, NvidiaGen::Modern),      // GTX 1650 (Turing)
            (0x2182, NvidiaGen::Modern),      // GTX 1660 Ti
            (0x1d81, NvidiaGen::Legacy580),   // Titan V (Volta)
            (0x1b80, NvidiaGen::Legacy580),   // GTX 1080
            (0x1c82, NvidiaGen::Legacy580),   // GTX 1050 Ti
            (0x13c2, NvidiaGen::Legacy580),   // GTX 970
            (0x1340, NvidiaGen::Legacy580),   // 830M (GM108)
            (0x1180, NvidiaGen::Unsupported), // GTX 680
            (0x0fc6, NvidiaGen::Unsupported), // GTX 650
        ];
        for (id, expected) in cases {
            assert_eq!(NvidiaGen::from_device_id(id), expected, "device {id:#06x}");
        }
    }

    #[test]
    fn pci_name_lookup_stays_inside_vendor() {
        let db = "# header\n10de  NVIDIA Corporation\n\t2684  AD102 [GeForce RTX 4090]\n\t1b80  GP104 [GeForce GTX 1080]\n1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\t2684  Not this one\n";
        assert_eq!(lookup_pci_name(db, 0x10de, 0x2684).as_deref(), Some("AD102 [GeForce RTX 4090]"));
        assert_eq!(lookup_pci_name(db, 0x10de, 0x9999), None);
        assert_eq!(lookup_pci_name(db, 0x1002, 0x2684).as_deref(), Some("Not this one"));
    }
}
