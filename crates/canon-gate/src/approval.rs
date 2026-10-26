//! External SSH signature verification for authenticated approvals.
//!
//! Verification is intentionally a subprocess boundary around the platform's
//! `ssh-keygen -Y verify`. The verifier never reads private keys, signs, or
//! trusts a persisted boolean/environment variable.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

/// Why an SSH signature could not be verified.
#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("signature namespace and signer must be non-empty trimmed values")]
    InvalidIdentity,
    #[error("allowed signers file is missing: {}", .0.display())]
    AllowedSignersMissing(PathBuf),
    #[error("clock unavailable: {0}")]
    Clock(#[from] SystemTimeError),
    #[error("write detached signature: {0}")]
    WriteSignature(#[source] std::io::Error),
    #[error("run ssh-keygen -Y verify: {0}")]
    RunVerifier(#[source] std::io::Error),
    /// `ssh-keygen` ran and refused; `detail` is its trimmed stderr.
    #[error("ssh signature verification failed: {detail}")]
    Rejected { detail: String },
}

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
) -> Result<(), ApprovalError> {
    if namespace.is_empty() || signer.trim().is_empty() || signer.trim() != signer {
        return Err(ApprovalError::InvalidIdentity);
    }
    if !allowed_signers.is_file() {
        return Err(ApprovalError::AllowedSignersMissing(allowed_signers.to_path_buf()));
    }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let signature_path = std::env::temp_dir().join(format!("canon-ssh-signature-{}-{nonce}", std::process::id()));
    std::fs::write(&signature_path, signature).map_err(ApprovalError::WriteSignature)?;
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
    let output = result.map_err(ApprovalError::RunVerifier)?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(ApprovalError::Rejected { detail: detail.trim().to_string() })
    }
}

/// Risk approvals use the stable domain-separated namespace.
pub fn verify_risk_approval(payload: &[u8], signature: &[u8], signer: &str, allowed_signers: &Path) -> Result<(), ApprovalError> {
    verify_ssh_signature(canon_model::APPROVAL_NAMESPACE, payload, signature, signer, allowed_signers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_or_untrimmed_identity_keeps_its_message() {
        let err = verify_ssh_signature("", b"p", b"s", "me", Path::new("signers")).unwrap_err();
        assert_eq!(err.to_string(), "signature namespace and signer must be non-empty trimmed values");
        let err = verify_ssh_signature("ns", b"p", b"s", " me", Path::new("signers")).unwrap_err();
        assert_eq!(err.to_string(), "signature namespace and signer must be non-empty trimmed values");
    }

    #[test]
    fn missing_allowed_signers_keeps_its_message() {
        let err = verify_risk_approval(b"p", b"s", "me", Path::new("/nonexistent/allowed_signers")).unwrap_err();
        assert_eq!(err.to_string(), "allowed signers file is missing: /nonexistent/allowed_signers");
    }

    #[test]
    fn io_and_verifier_failures_keep_their_messages_and_causes() {
        let io = || std::io::Error::other("boom");
        let write = ApprovalError::WriteSignature(io());
        assert_eq!(write.to_string(), "write detached signature: boom");
        assert!(std::error::Error::source(&write).is_some());
        let run = ApprovalError::RunVerifier(io());
        assert_eq!(run.to_string(), "run ssh-keygen -Y verify: boom");
        assert!(std::error::Error::source(&run).is_some());
        let rejected = ApprovalError::Rejected { detail: "Could not verify signature.".to_string() };
        assert_eq!(rejected.to_string(), "ssh signature verification failed: Could not verify signature.");
        let clock = ApprovalError::from(UNIX_EPOCH.duration_since(SystemTime::now()).unwrap_err());
        assert!(clock.to_string().starts_with("clock unavailable: "));
    }
}
