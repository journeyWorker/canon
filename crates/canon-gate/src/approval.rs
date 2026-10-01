//! External SSH signature verification for authenticated approvals.
//!
//! Verification is intentionally a subprocess boundary around the platform's
//! `ssh-keygen -Y verify`. The verifier never reads private keys, signs, or
//! trusts a persisted boolean/environment variable.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Verify an armored SSH detached signature over `payload` for `signer`.
///
/// `allowed_signers` is policy-controlled. A missing/unreadable file, missing
/// verifier, non-zero verifier exit, or malformed signature is an error.
pub fn verify_ssh_signature(
    namespace: &str,
    payload: &[u8],
    signature: &[u8],
    signer: &str,
    allowed_signers: &Path,
) -> Result<(), String> {
    if namespace.is_empty() || signer.trim().is_empty() || signer.trim() != signer {
        return Err("signature namespace and signer must be non-empty trimmed values".into());
    }
    if !allowed_signers.is_file() {
        return Err(format!("allowed signers file is missing: {}", allowed_signers.display()));
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock unavailable: {e}"))?
        .as_nanos();
    let signature_path = std::env::temp_dir().join(format!("canon-ssh-signature-{}-{nonce}", std::process::id()));
    std::fs::write(&signature_path, signature).map_err(|e| format!("write detached signature: {e}"))?;
    let result = Command::new("ssh-keygen")
        .args(["-Y", "verify", "-f"])
        .arg(allowed_signers)
        .args(["-I", signer, "-n", namespace, "-s"])
        .arg(&signature_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(payload)?;
            }
            child.wait_with_output()
        });
    let _ = std::fs::remove_file(&signature_path);
    let output = result.map_err(|e| format!("run ssh-keygen -Y verify: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(format!("ssh signature verification failed: {}", detail.trim()))
    }
}

/// Risk approvals use the stable domain-separated namespace.
pub fn verify_risk_approval(payload: &[u8], signature: &[u8], signer: &str, allowed_signers: &Path) -> Result<(), String> {
    verify_ssh_signature(canon_model::APPROVAL_NAMESPACE, payload, signature, signer, allowed_signers)
}
