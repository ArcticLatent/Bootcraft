//! Signed release manifests (`linux-release.json`), the format Arctic Latent
//! apps such as ComfyPilot publish with every release.
//!
//! The manifest lists each package with its SHA-256 and is signed with
//! Ed25519 over a domain-separated, NUL-delimited payload. A checksum file
//! published next to the downloads only proves the bytes match that file; the
//! signature proves the release came from the holder of the offline key.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::system::Distro;

const DOMAIN: &[u8] = b"arctic-linux-release-manifest\x00v1\x00";

/// Bootcraft's release-signing key (Ed25519, base64). Its private half is the
/// `ARCTIC_UPDATE_SIGNING_KEY` secret of Bootcraft's release workflow.
/// (ComfyPilot signs with its own key: `catalog::COMFYPILOT_PUBLIC_KEY`.)
pub const BOOTCRAFT_RELEASE_PUBLIC_KEY: &str = "sOpAOe+QCfoUQMNl/ZGpAKGaG4Pu2yaqteYbxqkwr5U=";

/// Bootcraft's own latest release, which it updates itself from.
pub const BOOTCRAFT_MANIFEST: &str =
    "https://github.com/ArcticLatent/Bootcraft/releases/latest/download/linux-release.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub tag: String,
    pub repository: String,
    pub assets: Vec<Asset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
    pub download_url: String,
}

/// The exact bytes the release key signs.
pub fn payload(manifest: &Manifest) -> Vec<u8> {
    let mut buf = DOMAIN.to_vec();
    for field in [&manifest.version, &manifest.tag, &manifest.repository] {
        buf.extend_from_slice(field.as_bytes());
        buf.push(0);
    }
    let mut assets: Vec<&Asset> = manifest.assets.iter().collect();
    assets.sort_by(|a, b| a.name.cmp(&b.name));
    for asset in assets {
        for field in [&asset.name, &asset.sha256, &asset.download_url] {
            buf.extend_from_slice(field.as_bytes());
            buf.push(0);
        }
    }
    buf
}

/// Parse a manifest and check its signature against `public_key` (base64).
pub fn verify(json: &str, public_key: &str) -> Result<Manifest, String> {
    let manifest: Manifest =
        serde_json::from_str(json).map_err(|e| format!("invalid release manifest: {e}"))?;
    let signature = manifest.signature.as_deref().ok_or("the release manifest is not signed")?;
    let key: [u8; 32] = STANDARD
        .decode(public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("invalid release public key")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "invalid release public key")?;
    let signature: [u8; 64] = STANDARD
        .decode(signature)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("malformed release signature")?;
    key.verify_strict(&payload(&manifest), &Signature::from_bytes(&signature))
        .map_err(|_| "the release manifest's signature does not match; refusing to install it")?;
    Ok(manifest)
}

/// The x86-64 package for this distribution, if the release has one.
pub fn asset_for(manifest: &Manifest, distro: Distro) -> Option<&Asset> {
    find_asset(manifest, |name| match distro {
        Distro::Arch => name.contains(".pkg.tar"),
        Distro::Fedora => name.ends_with(".rpm") && !name.ends_with(".src.rpm"),
        Distro::Ubuntu | Distro::Mint | Distro::Debian => name.ends_with(".deb"),
    })
}

/// The release's x86-64 AppImage, if it has one.
pub fn appimage_asset(manifest: &Manifest) -> Option<&Asset> {
    find_asset(manifest, |name| name.ends_with(".appimage"))
}

fn find_asset(manifest: &Manifest, kind: impl Fn(&str) -> bool) -> Option<&Asset> {
    manifest.assets.iter().find(|asset| {
        let name = asset.name.to_ascii_lowercase();
        let x86 = name.contains("x86_64") || name.contains("amd64");
        // The name becomes a file name in the download cache.
        let safe = !name.contains('/') && !name.starts_with('.');
        x86 && kind(&name) && safe && asset.download_url.starts_with("https://")
    })
}

/// Whether version `candidate` is later than `current`, comparing the
/// numbers of `major.minor.patch` (a leading `v` and a `-suffix` are ignored).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn numbers(version: &str) -> Vec<u64> {
        let version = version.trim().trim_start_matches('v');
        let core = version.split(['-', '+']).next().unwrap_or("");
        core.split('.').map(|part| part.parse().unwrap_or(0)).collect()
    }
    let (mut a, mut b) = (numbers(candidate), numbers(current));
    let len = a.len().max(b.len());
    a.resize(len, 0);
    b.resize(len, 0);
    a > b
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::catalog::COMFYPILOT_PUBLIC_KEY;

    const COMFYPILOT_1_0_0: &str = include_str!("../tests/fixtures/comfypilot-1.0.0-linux-release.json");

    /// Sign `manifest` with a throwaway key; returns (signed JSON, public key).
    pub(crate) fn sign(version: &str, assets: &[(&str, &str, &str)]) -> (String, String) {
        use ed25519_dalek::{Signer, SigningKey};
        let key = SigningKey::from_bytes(&[7; 32]);
        let mut manifest = Manifest {
            version: version.into(),
            tag: format!("v{version}"),
            repository: "ArcticLatent/Test".into(),
            assets: assets
                .iter()
                .map(|(name, sha, url)| Asset {
                    name: (*name).into(),
                    sha256: (*sha).into(),
                    download_url: (*url).into(),
                })
                .collect(),
            signature: None,
        };
        manifest.signature = Some(STANDARD.encode(key.sign(&payload(&manifest)).to_bytes()));
        let assets: Vec<_> = manifest
            .assets
            .iter()
            .map(|a| serde_json::json!({"name": a.name, "sha256": a.sha256, "download_url": a.download_url}))
            .collect();
        let json = serde_json::json!({
            "version": manifest.version, "tag": manifest.tag, "repository": manifest.repository,
            "assets": assets, "signature": manifest.signature,
        });
        (json.to_string(), STANDARD.encode(key.verifying_key().to_bytes()))
    }

    #[test]
    fn real_comfypilot_release_verifies() {
        let manifest = verify(COMFYPILOT_1_0_0, COMFYPILOT_PUBLIC_KEY).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        let pick = |d| asset_for(&manifest, d).map(|a| a.name.as_str());
        assert_eq!(pick(Distro::Arch), Some("comfypilot-1.0.0-1-x86_64.pkg.tar.zst"));
        assert_eq!(pick(Distro::Fedora), Some("comfypilot-1.0.0-1.fc43.x86_64.rpm"));
        assert_eq!(pick(Distro::Ubuntu), Some("comfypilot_1.0.0-1_amd64.deb"));
        assert_eq!(
            appimage_asset(&manifest).map(|a| a.name.as_str()),
            Some("ComfyPilot-1.0.0-x86_64.AppImage")
        );
    }

    #[test]
    fn bootcraft_release_key_is_a_real_key() {
        let key = STANDARD.decode(BOOTCRAFT_RELEASE_PUBLIC_KEY).expect("base64");
        let key: [u8; 32] = key.try_into().expect("32 bytes");
        assert!(VerifyingKey::from_bytes(&key).is_ok());
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v1.0", "0.9.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-rc1", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn tampering_is_detected() {
        let tampered = COMFYPILOT_1_0_0.replacen("39977443", "00000000", 1);
        assert!(verify(&tampered, COMFYPILOT_PUBLIC_KEY).unwrap_err().contains("does not match"));
        let (other, other_key) = sign("1.0.0", &[]);
        assert!(verify(&other, COMFYPILOT_PUBLIC_KEY).is_err(), "signed by a different key");
        assert!(verify(&other, &other_key).is_ok());
        let unsigned = r#"{"version":"1","tag":"v1","repository":"x","assets":[]}"#;
        assert_eq!(
            verify(unsigned, COMFYPILOT_PUBLIC_KEY).unwrap_err(),
            "the release manifest is not signed"
        );
    }

    #[test]
    fn unsafe_assets_are_never_selected() {
        let (json, key) = sign(
            "1",
            &[
                ("../evil-x86_64.pkg.tar.zst", "aa", "https://x/evil"),
                ("plain-x86_64.pkg.tar.zst", "bb", "http://insecure/plain"),
            ],
        );
        let manifest = verify(&json, &key).unwrap();
        assert_eq!(asset_for(&manifest, Distro::Arch), None);
    }
}
