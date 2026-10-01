//! Provider-neutral adapter response contract.
//!
//! This module is deliberately a read-only boundary. It validates responses
//! emitted by an execution provider, but never starts a provider, invokes a
//! tool, or enforces the declared capabilities. A provider sandbox must enforce
//! `capabilities` before execution.

use std::fs;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Current version of the provider-neutral response envelope.
pub const PROTOCOL_VERSION: u32 = 1;

/// A response's terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterStatus {
    Succeeded,
    Failed,
    Aborted,
}

/// Declared provider capabilities. These are records, not enforcement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterCapabilities {
    pub filesystem: FilesystemCapability,
    pub network: NetworkCapability,
    pub secrets: SecretsCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemCapability {
    ReadOnly,
    WorkspaceWrite,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkCapability {
    None,
    Allowlisted,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretsCapability {
    None,
    Brokered,
    Direct,
}

/// A content-addressed evidence reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub digest: String,
}

/// Telemetry associated with one adapter response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterTelemetry {
    pub started_at: String,
    pub ended_at: String,
    pub tool_calls: Option<u64>,
    pub retries: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

/// Versioned provider-neutral response envelope.
///
/// Unknown core fields are rejected. Provider-specific data belongs under
/// `extensions`, whose values are retained but never interpreted by canon.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterResponse {
    pub protocol_version: u32,
    pub run_id: String,
    pub provider: String,
    pub model: String,
    pub context_pack_id: String,
    pub status: AdapterStatus,
    pub capabilities: AdapterCapabilities,
    pub evidence_refs: Vec<EvidenceRef>,
    pub telemetry: AdapterTelemetry,
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAdapterResponse {
    protocol_version: u32,
    run_id: String,
    provider: String,
    model: String,
    context_pack_id: String,
    status: AdapterStatus,
    capabilities: AdapterCapabilities,
    evidence_refs: Vec<RawEvidenceRef>,
    telemetry: AdapterTelemetry,
    extensions: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvidenceRef {
    kind: String,
    #[serde(rename = "ref")]
    reference: String,
    digest: String,
}

impl<'de> Deserialize<'de> for AdapterResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawAdapterResponse::deserialize(deserializer)?;
        Self::try_from(raw).map_err(serde::de::Error::custom)
    }
}

/// Stable errors returned by response parsing and file validation.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("invalid adapter response JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("adapter response validation failed: {0}")]
    Validation(String),
    #[error("cannot read adapter response `{path}`: {source}")]
    Io { path: String, source: std::io::Error },
}

impl TryFrom<RawAdapterResponse> for AdapterResponse {
    type Error = AdapterError;

    fn try_from(raw: RawAdapterResponse) -> Result<Self, Self::Error> {
        if raw.protocol_version != PROTOCOL_VERSION {
            return Err(AdapterError::Validation(format!(
                "protocol_version must be {PROTOCOL_VERSION}"
            )));
        }
        validate_required("run_id", &raw.run_id)?;
        validate_provider(&raw.provider)?;
        validate_required("model", &raw.model)?;
        validate_digest("context_pack_id", &raw.context_pack_id)?;
        for (index, evidence) in raw.evidence_refs.iter().enumerate() {
            validate_evidence(index, evidence)?;
        }
        for (key, _) in &raw.extensions {
            if key.chars().any(char::is_control) {
                return Err(AdapterError::Validation(
                    "extensions contains a key with a control character".to_string(),
                ));
            }
        }
        validate_telemetry(&raw.telemetry)?;

        Ok(Self {
            protocol_version: raw.protocol_version,
            run_id: raw.run_id,
            provider: raw.provider,
            model: raw.model,
            context_pack_id: raw.context_pack_id,
            status: raw.status,
            capabilities: raw.capabilities,
            evidence_refs: raw
                .evidence_refs
                .into_iter()
                .map(|entry| EvidenceRef {
                    kind: entry.kind,
                    reference: entry.reference,
                    digest: entry.digest,
                })
                .collect(),
            telemetry: raw.telemetry,
            extensions: raw.extensions,
        })
    }
}

fn validate_required(label: &str, value: &str) -> Result<(), AdapterError> {
    if value.trim().is_empty() {
        return Err(AdapterError::Validation(format!("{label} must be non-empty")));
    }
    if value.chars().any(char::is_control) {
        return Err(AdapterError::Validation(format!("{label} contains a control character")));
    }
    Ok(())
}

fn validate_provider(provider: &str) -> Result<(), AdapterError> {
    validate_required("provider", provider)?;
    let known = matches!(provider, "claude" | "codex" | "omp" | "pi");
    if known || valid_slug(provider) {
        Ok(())
    } else {
        Err(AdapterError::Validation(
            "provider must be claude, codex, omp, pi, or a lowercase extension slug".to_string(),
        ))
    }
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes().first().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.as_bytes().last().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn validate_digest(label: &str, digest: &str) -> Result<(), AdapterError> {
    if digest.len() != 71
        || !digest.starts_with("sha256:")
        || !digest[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AdapterError::Validation(format!(
            "{label} must match sha256:<64 hexadecimal characters>"
        )));
    }
    Ok(())
}

fn validate_evidence(index: usize, evidence: &RawEvidenceRef) -> Result<(), AdapterError> {
    validate_required(&format!("evidence_refs[{index}].kind"), &evidence.kind)?;
    if !valid_kind(&evidence.kind) {
        return Err(AdapterError::Validation(format!(
            "evidence_refs[{index}].kind has invalid grammar"
        )));
    }
    validate_required(&format!("evidence_refs[{index}].ref"), &evidence.reference)?;
    if !safe_reference(&evidence.reference) {
        return Err(AdapterError::Validation(format!(
            "evidence_refs[{index}].ref is unsafe"
        )));
    }
    validate_digest(&format!("evidence_refs[{index}].digest"), &evidence.digest)
}

fn valid_kind(value: &str) -> bool {
    value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
}

fn safe_reference(value: &str) -> bool {
    if value.len() > 4096
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.starts_with('~')
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| part == "..")
    {
        return false;
    }
    // References are labels/relative paths, not dereferenceable resources.
    !value
        .split_once(':')
        .is_some_and(|(scheme, _)| !scheme.is_empty() && valid_slug(scheme))
        || value.starts_with("artifact:")
        || value.starts_with("run:")
        || value.starts_with("sha256:")
}

fn validate_telemetry(telemetry: &AdapterTelemetry) -> Result<(), AdapterError> {
    let started = parse_timestamp("telemetry.started_at", &telemetry.started_at)?;
    let ended = parse_timestamp("telemetry.ended_at", &telemetry.ended_at)?;
    if ended < started {
        return Err(AdapterError::Validation(
            "telemetry.ended_at must be greater than or equal to telemetry.started_at".to_string(),
        ));
    }
    if telemetry.cost_usd.is_some_and(|cost| !cost.is_finite() || cost < 0.0) {
        return Err(AdapterError::Validation(
            "telemetry.cost_usd must be finite and nonnegative".to_string(),
        ));
    }
    Ok(())
}

fn parse_timestamp(label: &str, value: &str) -> Result<DateTime<FixedOffset>, AdapterError> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|_| AdapterError::Validation(format!("{label} must be an RFC3339 timestamp")))
}

/// Parse and validate an adapter response from UTF-8 JSON.
///
/// The generic input accepts `&str`, `String`, `&[u8]`, and similar byte
/// containers without making callers allocate an intermediate string.
pub fn parse_response<T: AsRef<[u8]>>(input: T) -> Result<AdapterResponse, AdapterError> {
    Ok(serde_json::from_slice(input.as_ref())?)
}

/// A redacted, normalized summary safe for CLI output. Extension payloads are
/// intentionally absent; only their sorted key names are exposed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidationSummary {
    pub protocol_version: u32,
    pub run_id: String,
    pub provider: String,
    pub model: String,
    pub context_pack_id: String,
    pub status: AdapterStatus,
    pub capabilities: AdapterCapabilities,
    pub evidence_count: usize,
    pub telemetry: AdapterTelemetry,
    pub extension_keys: Vec<String>,
    pub context_join_verified: Option<bool>,
}

impl ValidationSummary {
    pub fn from_response(response: &AdapterResponse) -> Self {
        let mut extension_keys: Vec<_> = response.extensions.keys().cloned().collect();
        extension_keys.sort();
        Self {
            protocol_version: response.protocol_version,
            run_id: response.run_id.clone(),
            provider: response.provider.clone(),
            model: response.model.clone(),
            context_pack_id: response.context_pack_id.clone(),
            status: response.status,
            capabilities: response.capabilities.clone(),
            evidence_count: response.evidence_refs.len(),
            telemetry: response.telemetry.clone(),
            extension_keys,
            context_join_verified: None,
        }
    }
}

/// Read and validate one response file without executing anything.
pub fn validate_response<P: AsRef<Path>>(path: P) -> Result<ValidationSummary, AdapterError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| AdapterError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let response = parse_response(bytes)?;
    Ok(ValidationSummary::from_response(&response))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityLevel {
    None,
    ReadOnly,
    WorkspaceWrite,
    Allowlisted,
    Full,
    Brokered,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthorizationResult {
    pub execution_authorized: bool,
    pub execution_performed: bool,
    pub sandbox_enforced: bool,
    pub reasons: Vec<String>,
    pub capabilities: AdapterCapabilities,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityPolicy {
    #[serde(default)]
    runtime_enforced: bool,
    #[serde(default = "default_none")]
    filesystem: String,
    #[serde(default = "default_none")]
    network: String,
    #[serde(default = "default_none")]
    secrets: String,
}
fn default_none() -> String { "none".into() }

/// Authorize a validated response against `.canon/policy.yaml`; this never executes anything.
pub fn authorize_response<P: AsRef<Path>, R: AsRef<Path>>(response_path: P, repo: R) -> Result<AuthorizationResult, AdapterError> {
    let response = parse_response(fs::read(response_path.as_ref()).map_err(|source| AdapterError::Io { path: response_path.as_ref().display().to_string(), source })?)?;
    let policy_path = repo.as_ref().join(".canon/policy.yaml");
    let text = fs::read_to_string(&policy_path).unwrap_or_default();
    let root: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| AdapterError::Validation(format!("malformed policy: {e}")))?;
    let root = if root.is_null() { serde_yaml::Value::Mapping(Default::default()) } else { root };
    let root = root.as_mapping().ok_or_else(|| AdapterError::Validation("malformed policy: expected mapping".into()))?;
    let policy = root.get(&serde_yaml::Value::String("adapter_capabilities".into())).cloned().unwrap_or_else(|| serde_yaml::Value::Mapping(Default::default()));
    let policy: CapabilityPolicy = serde_yaml::from_value(policy).map_err(|e| AdapterError::Validation(format!("malformed adapter_capabilities policy: {e}")))?;
    let mut reasons = Vec::new();
    if !policy.runtime_enforced { reasons.push("runtime_enforced=false; no enforcement adapter is available".into()); }
    let fs_rank = |v: FilesystemCapability| match v { FilesystemCapability::None => 0, FilesystemCapability::ReadOnly => 1, FilesystemCapability::WorkspaceWrite => 2 };
    let net_rank = |v: NetworkCapability| match v { NetworkCapability::None => 0, NetworkCapability::Allowlisted => 1, NetworkCapability::Full => 2 };
    let sec_rank = |v: SecretsCapability| match v { SecretsCapability::None => 0, SecretsCapability::Brokered => 1, SecretsCapability::Direct => 2 };
    let fs_policy = match policy.filesystem.as_str() { "none" => 0, "read-only" => 1, "workspace-write" => 2, _ => { reasons.push("filesystem: invalid policy level".into()); 99 } };
    let net_policy = match policy.network.as_str() { "none" => 0, "allowlisted" => 1, "full" => 2, _ => { reasons.push("network: invalid policy level".into()); 99 } };
    let sec_policy = match policy.secrets.as_str() { "none" => 0, "brokered" => 1, "direct" => 2, _ => { reasons.push("secrets: invalid policy level".into()); 99 } };
    if fs_rank(response.capabilities.filesystem) > fs_policy { reasons.push(format!("filesystem: declared capability exceeds policy budget ({})", policy.filesystem)); }
    if net_rank(response.capabilities.network) > net_policy { reasons.push(format!("network: declared capability exceeds policy budget ({})", policy.network)); }
    if sec_rank(response.capabilities.secrets) > sec_policy { reasons.push(format!("secrets: declared capability exceeds policy budget ({})", policy.secrets)); }
    Ok(AuthorizationResult { execution_authorized: reasons.is_empty(), execution_performed: false, sandbox_enforced: false, reasons, capabilities: response.capabilities })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response() -> serde_json::Value {
        serde_json::json!({
            "protocol_version": 1,
            "run_id": "run-1",
            "provider": "claude",
            "model": "sonnet",
            "context_pack_id": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "status": "succeeded",
            "capabilities": {"filesystem": "read-only", "network": "none", "secrets": "none"},
            "evidence_refs": [{"kind": "test-run", "ref": "artifacts/result.json", "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}],
            "telemetry": {"started_at": "2026-01-01T00:00:00Z", "ended_at": "2026-01-01T00:01:00Z", "tool_calls": 1, "retries": 0, "tokens": 2, "cost_usd": 0.1},
            "extensions": {"claude": {"session_id": "s"}}
        })
    }

    #[test]
    fn absent_capability_policy_defaults_to_denial() {
        for policy in [None, Some(""), Some(" \n# no policy\n"), Some("null\n"), Some("~\n")] {
            let repo = tempfile::tempdir().unwrap();
            let response_path = repo.path().join("response.json");
            fs::write(&response_path, response().to_string()).unwrap();
            if let Some(policy) = policy {
                fs::create_dir(repo.path().join(".canon")).unwrap();
                fs::write(repo.path().join(".canon/policy.yaml"), policy).unwrap();
            }

            let result = authorize_response(&response_path, repo.path()).unwrap();
            assert!(!result.execution_authorized);
            assert!(!result.execution_performed);
            assert!(!result.sandbox_enforced);
            assert!(result.reasons.iter().any(|reason| reason.contains("runtime_enforced=false")));
            assert!(result.reasons.iter().any(|reason| reason.contains("filesystem: declared capability exceeds policy budget (none)")));
        }
    }

    #[test]
    fn rejects_non_mapping_capability_policy() {
        for policy in ["enabled\n", "- enabled\n", "[]\n", "false\n"] {
            let repo = tempfile::tempdir().unwrap();
            let response_path = repo.path().join("response.json");
            fs::write(&response_path, response().to_string()).unwrap();
            fs::create_dir(repo.path().join(".canon")).unwrap();
            fs::write(repo.path().join(".canon/policy.yaml"), policy).unwrap();

            let error = authorize_response(&response_path, repo.path()).unwrap_err();
            assert!(error.to_string().contains("malformed policy: expected mapping"));
        }
    }

    #[test]
    fn validates_and_hides_extension_payload() {
        let parsed = parse_response(response().to_string()).unwrap();
        let summary = ValidationSummary::from_response(&parsed);
        assert_eq!(summary.extension_keys, vec!["claude"]);
        assert_eq!(summary.context_join_verified, None);
    }

    #[test]
    fn rejects_extension_key_with_control_character() {
        let mut value = response();
        value["extensions"] = serde_json::json!({"provider\nforged": {"session_id": "s"}});

        let error = parse_response(value.to_string()).unwrap_err();
        assert!(error.to_string().contains("extensions contains a key with a control character"));
    }

    #[test]
    fn rejects_unknown_core_field() {
        let mut value = response();
        value["unexpected"] = serde_json::json!(true);
        assert!(parse_response(value.to_string()).is_err());
    }

    #[test]
    fn rejects_unsafe_reference() {
        let mut value = response();
        value["evidence_refs"][0]["ref"] = serde_json::json!("../secret");
        assert!(parse_response(value.to_string()).is_err());
    }

    #[test]
    fn accepts_extension_slug_provider() {
        let mut value = response();
        value["provider"] = serde_json::json!("my-provider");
        assert!(parse_response(value.to_string()).is_ok());
    }
}
