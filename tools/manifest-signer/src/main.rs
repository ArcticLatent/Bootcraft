//! Release-time signing for Bootcraft's `linux-release.json`.
//!
//! Never shipped: only `packaging/release-manifests.sh` runs it, in the
//! release workflow. It is the one place the private key is read; Bootcraft
//! itself only verifies, with the public key in `bootcraft_core::release`
//! (Arctic Latent's key, shared with ComfyPilot).
//!
//!   manifest-signer keygen
//!       A new Ed25519 key pair, both halves in base64.
//!   manifest-signer sign --manifest <path>
//!       Signs with ARCTIC_UPDATE_SIGNING_KEY (base64) and adds `signature`.
//!   manifest-signer verify --manifest <path> [--pubkey <base64>]
//!       Checks the signature against the built-in key (or --pubkey).

use std::process::ExitCode;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bootcraft_core::release::{self, ARCTIC_RELEASE_PUBLIC_KEY, Manifest};
use ed25519_dalek::{Signer, SigningKey};

const SIGNING_KEY_ENV: &str = "ARCTIC_UPDATE_SIGNING_KEY";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("keygen") => keygen(),
        Some("sign") => option(&args, "--manifest").and_then(|path| sign(&path)),
        Some("verify") => option(&args, "--manifest").and_then(|path| {
            let key = option(&args, "--pubkey").unwrap_or_else(|_| ARCTIC_RELEASE_PUBLIC_KEY.to_owned());
            verify(&path, &key)
        }),
        _ => Err("usage: manifest-signer keygen | sign --manifest <path> | verify --manifest <path> [--pubkey <base64>]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("manifest-signer: {message}");
            ExitCode::FAILURE
        }
    }
}

fn option(args: &[String], name: &str) -> Result<String, String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .ok_or(format!("{name} is missing"))
}

fn keygen() -> Result<(), String> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| format!("no randomness: {e}"))?;
    let key = SigningKey::from_bytes(&secret);
    println!("private (keep secret, {SIGNING_KEY_ENV}): {}", STANDARD.encode(key.to_bytes()));
    println!(
        "public (bootcraft_core::release::ARCTIC_RELEASE_PUBLIC_KEY): {}",
        STANDARD.encode(key.verifying_key().to_bytes())
    );
    Ok(())
}

fn read(path: &str) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: not a release manifest: {e}"))
}

fn sign(path: &str) -> Result<(), String> {
    let secret = std::env::var(SIGNING_KEY_ENV).map_err(|_| format!("{SIGNING_KEY_ENV} is not set"))?;
    let secret: [u8; 32] = STANDARD
        .decode(secret.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(format!("{SIGNING_KEY_ENV} is not a base64 Ed25519 private key"))?;
    let mut manifest = read(path)?;
    manifest.signature = None;
    let signature = SigningKey::from_bytes(&secret).sign(&release::payload(&manifest));
    manifest.signature = Some(STANDARD.encode(signature.to_bytes()));
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(path, json + "\n").map_err(|e| format!("{path}: {e}"))?;
    println!("signed {path}");
    Ok(())
}

fn verify(path: &str, public_key: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let manifest = release::verify(&text, public_key)?;
    println!(
        "verified {path}: {} {} ({} files)",
        manifest.repository,
        manifest.version,
        manifest.assets.len()
    );
    Ok(())
}
