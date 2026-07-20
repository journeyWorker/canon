//! `GitTier`: Hive-partitioned, append-only files under the consumer
//! repo (design D2, tier-adapter-trait spec, git-tier-layout-enforcement
//! spec) — the only tier that MUST work with zero network (design §9
//! local-first) and is fully offline-tested.

use std::path::{Path, PathBuf};

use canon_model::envelope::RecordKind;
use canon_model::evidence::{EvidenceViolation, RawRecord};
use canon_model::FailureClass;

use crate::partition::{expected_relative_path, validate_body, validate_kind_matches_content};
use crate::policy::Backend;
use crate::tier::{AgeReport, AgingRule, StoreError, StoredRecord, Tier, TierQuery, TierReadResult, WriteReceipt};

/// [`GitTier::scan_kind_where`]/[`GitTier::scan_namespaced_kind`]'s
/// return shape: kept `(path, record)` pairs plus every violation
/// encountered along the scan.
pub type ScanResult = (Vec<(PathBuf, RawRecord)>, Vec<EvidenceViolation>);

/// A `kind=<x>/` directory [`GitTier::scan_corpus`] found whose `<x>`
/// does not match any of `RecordKind`'s twelve closed core kinds
/// (scenario-spine-layout spec) — skipped wholesale, reported here
/// rather than descended into or folded into `violations`. The
/// forward-compat seam that lets a future s16 plugin kind coexist
/// without breaking an s15 consumer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignNamespace {
    /// The `kind=<x>/` directory's `<x>` portion.
    pub kind: String,
}

/// [`GitTier::scan_corpus`]'s return shape: every recognized kind's
/// records (each tagged with its own [`RecordKind`], since this scan
/// spans all twelve kinds at once, unlike [`Tier::read`]'s per-kind
/// query) and violations — exactly what per-kind `scan_kind_where`
/// would have produced, aggregated — plus every unrecognized
/// `kind=<x>/` directory encountered along the way.
#[derive(Debug, Clone, Default)]
pub struct CorpusScanResult {
    pub records: Vec<(RecordKind, PathBuf, RawRecord)>,
    pub violations: Vec<EvidenceViolation>,
    pub foreign_namespaces: Vec<ForeignNamespace>,
}

/// [`GitTier::write_namespaced`]'s outcome -- [`WriteReceipt`]'s
/// namespaced-kind analog: same `location`/`digest`/`deduped` shape, but
/// `namespaced_kind: String` in place of `kind: RecordKind` (s16
/// design.md D1: an overlay's on-disk kind is "a plain string, never a
/// `RecordKind` variant", which `WriteReceipt.kind`'s closed
/// `RecordKind` type structurally cannot represent). A distinct type
/// rather than widening `WriteReceipt.kind`'s type: `WriteReceipt` is
/// shared by every `Tier::write` implementation across all three tiers
/// (git/pg/r2) for the twelve CLOSED core kinds -- widening it would
/// touch the frozen typed `Tier::write`/`read` core path this change's
/// own non-goals protect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespacedWriteReceipt {
    pub namespaced_kind: String,
    /// The git-tier-relative location the record was (or already was)
    /// stored at -- `kind={namespaced_kind}/{natural_key}__{digest12}.json`.
    pub location: String,
    pub digest: String,
    /// `true` when this write found the identical digest already
    /// present at the resolved location and performed no new write --
    /// UNLIKE `WriteReceipt`'s own doc comment ("`GitTier::write` never
    /// sets this: a git-tier duplicate-path write is a hard error
    /// instead"), `write_namespaced` DOES dedup on an exact path match:
    /// since the path's own suffix IS the content digest
    /// (`partition.rs` module doc), a path that already exists is, by
    /// construction, byte-identical content (the same reasoning
    /// `R2Tier::write` documents) -- so a repeatable overlay-sync run
    /// (`canon plugin sync`, out of this change's scope) writes zero
    /// new records over an already-synced corpus, never erroring.
    pub deduped: bool,
}

/// F1's store-layer defense-in-depth: the namespaced-kind generalization
/// of [`crate::partition::validate_kind_matches_content`]'s
/// directory/content-kind invariant for the twelve closed core kinds --
/// an overlay body's OWN top-level `kind` field must equal
/// `namespaced_kind`, the `kind=<namespaced_kind>/` directory a caller
/// (`write_namespaced` on write, `scan_namespaced_kind` on scan) already
/// knows this record either belongs to or is about to be written under.
/// A misdeclared/mismatched `kind` is a `layout` violation, exactly the
/// same subject/class core's own check reports for the twelve closed
/// kinds -- an overlay can never live under `kind=<ns>.<kind>/` while
/// its own body claims a different kind, at either half of this
/// primitive.
fn validate_namespaced_kind_matches_content(namespaced_kind: &str, json: &serde_json::Value) -> Result<(), EvidenceViolation> {
    match json.get("kind").and_then(serde_json::Value::as_str) {
        Some(found) if found == namespaced_kind => Ok(()),
        Some(found) => Err(EvidenceViolation::new(
            FailureClass::Malformed,
            "layout",
            format!("directory `kind={namespaced_kind}/` but overlay body's own `kind` is `{found}`"),
        )),
        None => Err(EvidenceViolation::new(FailureClass::Malformed, "kind", "missing or non-string `kind` field")),
    }
}

pub struct GitTier {
    root: PathBuf,
}

impl GitTier {
    /// `root` is `canon.yaml`'s `tiers.git.root` (design D3), an
    /// ordinary directory on local disk — no network, no credentials,
    /// works identically in a fixture tmpdir or a real consumer repo
    /// checkout.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn absolute(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }

    /// Scan every file under `kind={kind}/`, validating layout+body
    /// exactly as [`Tier::read`]/[`Tier::age`] need, keeping only
    /// records whose `at` satisfies `keep`. Returns each kept record
    /// alongside its git-tier-relative path (so [`Tier::age`] can
    /// delete the exact file it just moved) plus every violation
    /// encountered — the one scan loop both callers share.
    fn scan_kind_where(
        &self,
        kind: RecordKind,
        keep: impl Fn(chrono::DateTime<chrono::Utc>) -> bool,
    ) -> Result<ScanResult, StoreError> {
        let mut records = Vec::new();
        let mut violations = Vec::new();
        let kind_dir = self.root.join(format!("kind={}", kind.as_str()));
        if !kind_dir.exists() {
            return Ok((records, violations));
        }
        for entry in walkdir::WalkDir::new(&kind_dir).sort_by_file_name().into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let absolute = entry.path();
            let relative = absolute.strip_prefix(&self.root).expect("walked entries are under root").to_path_buf();

            let bytes = std::fs::read(absolute)?;
            let json: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    violations.push(EvidenceViolation::new(FailureClass::Malformed, relative.display().to_string(), e.to_string()));
                    continue;
                }
            };

            if let Err(violation) = validate_kind_matches_content(kind, &json) {
                violations.push(violation);
                continue;
            }

            let expected = match expected_relative_path(kind, &json) {
                Ok(p) => p,
                Err(violation) => {
                    violations.push(violation);
                    continue;
                }
            };
            if expected != relative {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    "layout",
                    format!("expected `{}` (template `{}`), found `{}`", expected.display(), kind.partition_template(), relative.display()),
                ));
                continue;
            }

            let raw = RawRecord(json);
            if let Err(violation) = validate_body(kind, &raw) {
                violations.push(violation);
                continue;
            }

            let at = crate::tier::raw_record_at(&raw);
            if keep(at) {
                records.push((relative, raw));
            }
        }
        Ok((records, violations))
    }

    /// Corpus-wide scan across every `kind=<x>/` directory directly
    /// under `root` — unlike [`Tier::read`]/`scan_kind_where`, which
    /// are always told a specific, already-known `kind` up front,
    /// this walks whatever kind directories actually exist on disk
    /// (scenario-spine-layout spec: "An unrecognized kind=<x>/
    /// directory is skipped and reported as foreign-namespace"). A
    /// directory name that parses to one of the twelve closed
    /// `RecordKind`s is scanned exactly as `scan_kind_where` would —
    /// task 2.3's "twelve core kinds are scanned exactly as before"
    /// — one that does NOT is skipped wholesale (never descended
    /// into, never contributing a `malformed` violation) and
    /// reported as a [`ForeignNamespace`] instead. The forward-compat
    /// reader rule that keeps a future s16 plugin kind from breaking
    /// an s15-era consumer.
    pub fn scan_corpus(&self) -> Result<CorpusScanResult, StoreError> {
        let mut result = CorpusScanResult::default();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(e) => return Err(e.into()),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(kind_str) = name.strip_prefix("kind=") else { continue };
            match RecordKind::ALL.into_iter().find(|k| k.as_str() == kind_str) {
                Some(kind) => {
                    let (found, violations) = self.scan_kind_where(kind, |_| true)?;
                    result.records.extend(found.into_iter().map(|(path, raw)| (kind, path, raw)));
                    result.violations.extend(violations);
                }
                None => result.foreign_namespaces.push(ForeignNamespace { kind: kind_str.to_string() }),
            }
        }
        Ok(result)
    }

    /// Generalizes [`Tier::write`]'s digest-suffix + append-only
    /// algorithm off an arbitrary namespaced-kind STRING instead of the
    /// closed [`RecordKind`] enum (s16 design.md D4, tasks.md 2.1) --
    /// the storage primitive a plugin-aware writer (`canon-plugin`, out
    /// of this crate) targets for an overlay record whose on-disk kind
    /// (`<namespace>.<kind>`) is a plugin-declared string, never one of
    /// the twelve core kinds. Path:
    /// `kind={namespaced_kind}/{natural_key}__{digest12}.json`.
    ///
    /// `natural_key` is NOT re-derived here (unlike `write`'s
    /// `resolve_partition`, which extracts a record's natural key from
    /// its OWN content via a per-`RecordKind` match arm) -- this
    /// function has no schema for an arbitrary namespaced kind, so the
    /// caller derives it from the validated body's own join-key field
    /// values FIRST (design.md D4: "mirroring canon-store core's
    /// `resolve_partition`, which derives a record's on-disk path FROM
    /// the record's own fields, never from an out-of-band argument").
    /// [`crate::partition::natural_key_matches_body`] is this
    /// function's OWN defense-in-depth check that the supplied
    /// `natural_key` genuinely traces back to `body`'s own content --
    /// generalized WITHOUT requiring the join-key field NAMES this
    /// function (unlike its canon-plugin caller) never receives.
    ///
    /// REJECTS, loud, BEFORE constructing any filesystem path: a
    /// `namespaced_kind` colliding with a core `RecordKind`, a
    /// `namespaced_kind`/`natural_key` failing the path-safety grammar,
    /// a `natural_key` disagreeing with `body`'s own content (tasks.md
    /// 2.3), or `body`'s own top-level `kind` field disagreeing with
    /// `namespaced_kind` itself (F1's store-layer defense in depth,
    /// [`validate_namespaced_kind_matches_content`]).
    ///
    /// An EXISTING resolved path is NOT automatically an error here
    /// (unlike `write`'s "a git-tier duplicate-path write is a hard
    /// error" -- git-tier-layout-enforcement spec): since the path's own
    /// suffix IS the content digest (`partition.rs` module doc), a path
    /// that already exists is EXPECTED to be byte-identical content
    /// (the same reasoning `R2Tier::write` documents), so a genuinely
    /// matching resubmission reports `deduped: true` and performs no new
    /// write -- the idempotence a repeatable overlay-sync run needs
    /// (design.md testing section: "run twice over an unchanged
    /// inventory writes zero new overlay records"). That expectation is
    /// VERIFIED, never assumed: the existing file's own bytes are read
    /// and compared against the incoming `body` before this function
    /// ever reports success over them -- a 12-hex-digest collision or a
    /// hand-tampered file at the SAME path holding DIFFERENT content is
    /// a loud `StoreError::Layout`, never a silent `deduped: true`.
    pub fn write_namespaced(&self, namespaced_kind: &str, natural_key: &str, body: RawRecord) -> Result<NamespacedWriteReceipt, StoreError> {
        crate::partition::validate_namespaced_kind(namespaced_kind)?;
        crate::partition::validate_natural_key(natural_key)?;
        crate::partition::natural_key_matches_body(natural_key, &body.0)?;
        validate_namespaced_kind_matches_content(namespaced_kind, &body.0)?;

        let digest = crate::partition::content_digest12(&body.0);
        let relative = PathBuf::from(format!("kind={namespaced_kind}/{natural_key}__{digest}.json"));
        let absolute = self.absolute(&relative);

        // The canonical bytes this write would produce. `serde_json::Value`
        // is a `BTreeMap` workspace-wide (no `preserve_order` feature), so
        // `to_vec_pretty` is DETERMINISTIC for a given Value -- the same
        // sorted-key form `content_digest12` hashes over. Dedup compares and
        // the write emits exactly these bytes, so a genuine idempotent
        // resubmission is always byte-identical while any file at the path
        // that is NOT the canonical serialization (a 12-hex digest collision,
        // or a tampered / hand-edited / non-canonical file) is caught.
        let canonical = serde_json::to_vec_pretty(&body.0)?;

        if absolute.exists() {
            let existing_bytes = std::fs::read(&absolute)?;
            if existing_bytes != canonical {
                return Err(StoreError::Layout(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    format!(
                        "`{digest}`-suffixed path already holds bytes that are NOT this body's canonical serialization -- a 12-hex digest collision, or a tampered / non-canonical file, never silently deduped"
                    ),
                )));
            }
            return Ok(NamespacedWriteReceipt {
                namespaced_kind: namespaced_kind.to_string(),
                location: relative.display().to_string(),
                digest,
                deduped: true,
            });
        }

        crate::atomic::write_atomic(&absolute, &canonical)?;

        Ok(NamespacedWriteReceipt { namespaced_kind: namespaced_kind.to_string(), location: relative.display().to_string(), digest, deduped: false })
    }

    /// [`GitTier::write_namespaced`]'s read-side twin (s16 tasks.md
    /// 2.2), mirroring [`GitTier::scan_kind_where`]'s walk over an
    /// arbitrary namespaced-kind STRING instead of a closed
    /// [`RecordKind`]. Rejects the SAME `namespaced_kind` collision/
    /// grammar violations `write_namespaced` does (tasks.md 2.3, defense
    /// in depth) before touching disk.
    ///
    /// Unlike `scan_kind_where`, this cannot validate a found record's
    /// full expected relative path (that needs `resolve_partition`'s
    /// per-`RecordKind` natural-key extraction, unavailable for an
    /// arbitrary namespaced kind) -- instead it validates the THREE
    /// things that generalize without a schema, every one of them the
    /// CANONICAL path shape `write_namespaced` itself constructs
    /// (`kind={namespaced_kind}/{natural_key}__{digest12}.json`, no
    /// nested subdirectory): (1) the path is FLAT, exactly one
    /// component below `kind={namespaced_kind}/` -- a nested path (e.g.
    /// an `attic/` subdirectory) is never a record `write_namespaced`
    /// could have produced; (2) the file extension is `.json`; (3) the
    /// filename's own `__{digest12}` suffix is present, exactly 12
    /// lowercase-hex characters, AND agrees with `content_digest12` of
    /// the file's OWN parsed content -- the same self-consistency
    /// `write_namespaced` itself just wrote. F1's store-layer defense
    /// in depth ([`validate_namespaced_kind_matches_content`]) also
    /// runs here: a found record whose own body `kind` disagrees with
    /// `namespaced_kind` is excluded too. Any of these failing is
    /// reported as a violation and skipped -- never aborting the rest
    /// of the scan (mirrors `scan_kind_where`'s own skip-not-crash
    /// discipline).
    pub fn scan_namespaced_kind(&self, namespaced_kind: &str) -> Result<ScanResult, StoreError> {
        crate::partition::validate_namespaced_kind(namespaced_kind)?;

        let mut records = Vec::new();
        let mut violations = Vec::new();
        let kind_dir = self.root.join(format!("kind={namespaced_kind}"));
        if !kind_dir.exists() {
            return Ok((records, violations));
        }
        for entry in walkdir::WalkDir::new(&kind_dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let absolute = entry.path();
            let relative = absolute.strip_prefix(&self.root).expect("walked entries are under root").to_path_buf();

            // F3: the canonical overlay path shape `write_namespaced`
            // writes is FLAT (`kind={namespaced_kind}/{file}.json`, no
            // nested subdirectory below the kind dir) -- a path this
            // shape check rejects can never have come from
            // `write_namespaced` itself, so it is a layout violation,
            // never a candidate record, before its content is even
            // read.
            if relative.components().count() != 2 {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    "overlay path is nested under its kind directory -- write_namespaced only ever writes one flat level deep".to_string(),
                ));
                continue;
            }
            if absolute.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    "overlay path does not end in `.json`".to_string(),
                ));
                continue;
            }

            let bytes = std::fs::read(absolute)?;
            let json: serde_json::Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    violations.push(EvidenceViolation::new(FailureClass::Malformed, relative.display().to_string(), e.to_string()));
                    continue;
                }
            };

            if let Err(violation) = validate_namespaced_kind_matches_content(namespaced_kind, &json) {
                violations.push(violation);
                continue;
            }

            let digest = crate::partition::content_digest12(&json);
            let stem = absolute.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let Some((_, suffix)) = stem.rsplit_once("__") else {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    "filename carries no `__<digest12>` suffix".to_string(),
                ));
                continue;
            };
            let is_lower_hex12 = suffix.len() == 12 && suffix.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if !is_lower_hex12 {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    format!("filename digest suffix `{suffix}` is not 12 lowercase-hex characters"),
                ));
                continue;
            }
            if suffix != digest {
                violations.push(EvidenceViolation::new(
                    FailureClass::Malformed,
                    relative.display().to_string(),
                    format!("filename digest suffix disagrees with content digest `{digest}`"),
                ));
                continue;
            }

            records.push((relative, RawRecord(json)));
        }
        Ok((records, violations))
    }
}

impl Tier for GitTier {
    fn backend(&self) -> Backend {
        Backend::Git
    }

    fn write(&self, record: &dyn StoredRecord) -> Result<WriteReceipt, StoreError> {
        let kind = record.kind();
        let raw = record.to_raw();
        let relative = expected_relative_path(kind, &raw.0)?;
        let absolute = self.absolute(&relative);

        if absolute.exists() {
            return Err(StoreError::DuplicatePath { kind, location: relative.display().to_string() });
        }

        // NOT a live defense-in-depth check, despite appearances:
        // `relative` above was JUST derived by calling
        // `expected_relative_path(kind, &raw.0)` on this exact
        // `(kind, json)` pair, and `validate_layout` internally does
        // nothing but call that SAME pure function again and compare
        // the result to `relative` — same function, same inputs,
        // deterministic output, so this can never actually disagree
        // or fail here. A bug in `expected_relative_path` itself
        // would reproduce identically on both calls, not get caught
        // by this. What it actually is: a self-documenting invariant
        // marker (this IS the path `read`'s own layout check will
        // expect) and a cheap regression net against a future edit
        // that changes `relative`'s derivation above without updating
        // this call site to match.
        crate::partition::validate_layout(kind, &relative, &raw.0)?;

        crate::atomic::write_atomic(&absolute, &serde_json::to_vec_pretty(&raw.0)?)?;

        Ok(WriteReceipt {
            kind,
            location: relative.display().to_string(),
            digest: crate::partition::content_digest12(&raw.0),
            deduped: false,
        })
    }

    fn read(&self, query: &TierQuery) -> Result<TierReadResult, StoreError> {
        let since = query.since;
        let (found, violations) = self.scan_kind_where(query.kind, move |at| since.is_none_or(|s| at >= s))?;
        Ok(TierReadResult { records: found.into_iter().map(|(_, raw)| raw).collect(), violations })
    }

    /// A genuine move-and-delete, exercised whenever a repo's own
    /// `canon.yaml` routes/ages a kind through `git` (S2's OWN shipped
    /// `canon.yaml` never does — git-tier kinds are authored/promoted
    /// content, routed there permanently — but `Tier::age` is a
    /// required capability on every adapter, not just the two design
    /// context expects to age in practice). Deleting an aged-out
    /// git-tier file is NOT an append-only violation: append-only
    /// forbids *overwriting* a path with different content; removing a
    /// file once its content is confirmed durable at the destination
    /// tier is the sanctioned aging mutation (tier-policy spec).
    fn age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError> {
        let cutoff = chrono::Utc::now() - rule.after;
        let (candidates, _violations) = self.scan_kind_where(rule.kind, |at| at < cutoff)?;

        let mut moved = 0;
        let mut already_aged = 0;
        for (relative, raw) in candidates {
            let receipt = rule.destination.write(&crate::tier::RawWrite(raw))?;
            if receipt.deduped {
                already_aged += 1;
            } else {
                moved += 1;
            }
            std::fs::remove_file(self.absolute(&relative))?;
        }
        Ok(AgeReport { kind: rule.kind, moved, already_aged })
    }
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope, RecordKind};
    use canon_model::ids::{ChangeId, ProjectId, RoleId, ScenarioId, SpecDigest};
    use canon_model::records::{Change, ChangeStatus, Scenario};
    use chrono::Utc;

    use super::*;

    fn actor() -> Actor {
        Actor::new("test-agent", RoleId::parse("implementer").unwrap())
    }

    #[test]
    fn write_then_read_round_trips_a_flat_kind() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let change = Change::new(
            Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
            ChangeId::parse("s2-tiered-storage").unwrap(),
            "S2",
            "tiered storage",
            ChangeStatus::InProgress,
        );
        let receipt = tier.write(&change).unwrap();
        assert!(!receipt.deduped);
        assert!(receipt.location.starts_with("kind=change/"));

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert!(result.violations.is_empty(), "unexpected violations: {:?}", result.violations);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].0["change_id"], "s2-tiered-storage");
    }

    #[test]
    fn write_then_read_round_trips_an_area_scoped_kind_area_from_scenario_id() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let scenario = Scenario::new(
            Envelope::new(1, RecordKind::Scenario, Utc::now(), actor()),
            ProjectId::parse("root").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            "hotdeal",
            "",
            SpecDigest::of(b"fixture .feature bytes"),
        );
        let receipt = tier.write(&scenario).unwrap();
        assert!(receipt.location.starts_with("kind=scenario/area=world/"), "got {}", receipt.location);

        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1);
    }

    #[test]
    fn duplicate_write_is_rejected_not_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let change = Change::new(envelope.clone(), ChangeId::parse("s2-tiered-storage").unwrap(), "S2", "x", ChangeStatus::Proposed);
        tier.write(&change).unwrap();
        let err = tier.write(&change).unwrap_err();
        assert!(matches!(err, StoreError::DuplicatePath { .. }), "expected DuplicatePath, got {err:?}");
    }

    #[test]
    fn a_correction_with_different_content_is_a_new_append_not_a_collision() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let id = ChangeId::parse("s2-tiered-storage").unwrap();
        let e1 = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let e2 = Envelope::new(1, RecordKind::Change, Utc::now(), actor());
        let first = Change::new(e1, id.clone(), "S2", "draft", ChangeStatus::Proposed);
        let second = Change::new(e2, id, "S2", "draft", ChangeStatus::InProgress);
        tier.write(&first).unwrap();
        tier.write(&second).unwrap(); // different `status` => different digest => different path, not rejected
        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 2);
    }

    #[test]
    fn since_filter_excludes_older_records() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let old_at = Utc::now() - chrono::Duration::days(10);
        let new_at = Utc::now();
        let old = Change::new(
            Envelope::new(1, RecordKind::Change, old_at, actor()),
            ChangeId::parse("old-change").unwrap(),
            "old",
            "x",
            ChangeStatus::Completed,
        );
        let new = Change::new(
            Envelope::new(1, RecordKind::Change, new_at, actor()),
            ChangeId::parse("new-change").unwrap(),
            "new",
            "x",
            ChangeStatus::Proposed,
        );
        tier.write(&old).unwrap();
        tier.write(&new).unwrap();
        let cutoff = Utc::now() - chrono::Duration::days(1);
        let result = tier.read(&TierQuery::kind(RecordKind::Change).since(cutoff)).unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].0["change_id"], "new-change");
    }

    #[test]
    fn scan_order_is_sorted_lexicographically_not_creation_order() {
        // s21 D4/spec `cross-tier-supersession`: writing `zzz-change`
        // BEFORE `aaa-change` (reverse-lexicographic creation order) —
        // if the scan trusted raw filesystem directory-entry order it
        // would very likely return them in that same, non-sorted,
        // creation order. The sorted scan must return `aaa-change`
        // first regardless of which one was written first.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let zzz = Change::new(Envelope::new(1, RecordKind::Change, Utc::now(), actor()), ChangeId::parse("zzz-change").unwrap(), "zzz", "x", ChangeStatus::Proposed);
        let aaa = Change::new(Envelope::new(1, RecordKind::Change, Utc::now(), actor()), ChangeId::parse("aaa-change").unwrap(), "aaa", "x", ChangeStatus::Proposed);
        tier.write(&zzz).unwrap();
        tier.write(&aaa).unwrap();

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(result.records.len(), 2);
        let ids: Vec<&str> = result.records.iter().map(|r| r.0["change_id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["aaa-change", "zzz-change"], "sorted (lexicographic-by-path) order, never creation order");

        // Two scans of an unchanged directory must agree byte-for-byte.
        let second_read = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        let second_ids: Vec<&str> = second_read.records.iter().map(|r| r.0["change_id"].as_str().unwrap()).collect();
        assert_eq!(ids, second_ids, "two scans of an unchanged directory must return the identical order both times");
    }

    #[test]
    fn a_preexisting_misfiled_file_is_flagged_on_read_and_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        // Area-scoped `scenario` record planted FLAT (no `area=`
        // segment) — a Hive-nested layout is required.
        let bad_dir = dir.path().join("kind=scenario");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            bad_dir.join("world.firstbuy-hotdeal.26.json"),
            serde_json::json!({
                "schema": 1, "kind": "scenario", "at": Utc::now().to_rfc3339(),
                "actor": {"agent_id": "a", "role": "implementer"},
                "project_id": "root", "scenario_id": "world.firstbuy-hotdeal.26", "title": "t"
            })
            .to_string(),
        )
        .unwrap();

        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert!(result.records.is_empty(), "misfiled record must be excluded from records");
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].class, FailureClass::Malformed);
    }

    #[test]
    fn area_mismatch_between_directory_and_content_is_a_layout_violation() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        // Directory says `area=world`, but content's scenario_id area
        // is `promise-date` — exactly the parity-harness six-mismatch
        // gotcha, planted directly.
        let bad_dir = dir.path().join("kind=scenario").join("area=world");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            bad_dir.join("promise-date.play.03__aaaaaaaaaaaa.json"),
            serde_json::json!({
                "schema": 1, "kind": "scenario", "at": Utc::now().to_rfc3339(),
                "actor": {"agent_id": "a", "role": "implementer"},
                "project_id": "root", "scenario_id": "promise-date.play.03", "title": "t"
            })
            .to_string(),
        )
        .unwrap();

        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert!(result.records.is_empty());
        assert_eq!(result.violations.len(), 1);
        assert!(result.violations[0].detail.contains("area=promise-date"), "detail: {}", result.violations[0].detail);
    }

    #[test]
    fn malformed_json_is_skipped_not_crashed_on() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let kind_dir = dir.path().join("kind=change");
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("not-json__abcdef123456.json"), "{not valid json").unwrap();

        let result = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert!(result.records.is_empty());
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn scan_corpus_skips_and_reports_an_unrecognized_kind_directory_never_as_malformed() {
        // scenario-spine-layout spec: "An unrecognized kind=<x>/
        // directory is skipped and reported as foreign-namespace" —
        // the forward-compat seam for a future s16 plugin kind.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let foreign_dir = dir.path().join("kind=plugin-widget");
        std::fs::create_dir_all(&foreign_dir).unwrap();
        std::fs::write(foreign_dir.join("some-record.json"), "{not even valid json").unwrap();

        let change = Change::new(
            Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
            ChangeId::parse("s2-tiered-storage").unwrap(),
            "S2",
            "x",
            ChangeStatus::Proposed,
        );
        tier.write(&change).unwrap();

        let result = tier.scan_corpus().unwrap();
        assert_eq!(result.foreign_namespaces, vec![ForeignNamespace { kind: "plugin-widget".to_string() }]);
        assert!(result.violations.is_empty(), "an unrecognized kind dir must never contribute a malformed violation: {:?}", result.violations);
        assert_eq!(result.records.len(), 1, "the one recognized `change` record must still be scanned normally");
        assert_eq!(result.records[0].0, RecordKind::Change);
    }

    #[test]
    fn scan_corpus_still_fails_a_malformed_record_under_a_known_kind() {
        // Only an UNRECOGNIZED kind directory is skipped+reported — a
        // malformed record under a recognized `kind=<x>/` directory
        // still fails exactly as before (spec: "the new rule changes
        // behavior ONLY for unrecognized kind directories").
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let kind_dir = dir.path().join("kind=change");
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("not-json__abcdef123456.json"), "{not valid json").unwrap();

        let result = tier.scan_corpus().unwrap();
        assert!(result.foreign_namespaces.is_empty(), "a known kind must never be reported as foreign-namespace");
        assert!(result.records.is_empty());
        assert_eq!(result.violations.len(), 1, "a malformed record under a known kind must still fail loud");
        assert_eq!(result.violations[0].class, FailureClass::Malformed);
    }

    #[test]
    fn scan_corpus_scans_every_recognized_kind_exactly_as_a_per_kind_read_would() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let change = Change::new(
            Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
            ChangeId::parse("s2-tiered-storage").unwrap(),
            "S2",
            "x",
            ChangeStatus::Proposed,
        );
        let scenario = Scenario::new(
            Envelope::new(1, RecordKind::Scenario, Utc::now(), actor()),
            ProjectId::parse("root").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            "hotdeal",
            "",
            SpecDigest::of(b"fixture .feature bytes"),
        );
        tier.write(&change).unwrap();
        tier.write(&scenario).unwrap();

        let result = tier.scan_corpus().unwrap();
        assert!(result.violations.is_empty());
        assert!(result.foreign_namespaces.is_empty());
        assert_eq!(result.records.len(), 2);
        assert!(result.records.iter().any(|(kind, _, _)| *kind == RecordKind::Change));
        assert!(result.records.iter().any(|(kind, _, _)| *kind == RecordKind::Scenario));
    }

    /// A well-formed `porting.coverage`-shaped overlay body -- canon-store's
    /// own tests never depend on `canon-plugin`, so this is a plain
    /// hand-built JSON body, not an `OverlayEnvelope`/`OverlayDecl`.
    fn overlay_body(project_id: &str, scenario_id: &str, covered: bool) -> RawRecord {
        RawRecord(serde_json::json!({
            "schema": 1,
            "kind": "porting.coverage",
            "at": Utc::now().to_rfc3339(),
            "actor": {"agent_id": "porting-sync", "role": "implementer"},
            "project_id": project_id,
            "scenario_id": scenario_id,
            "covered": covered,
        }))
    }

    #[test]
    fn write_namespaced_then_scan_namespaced_kind_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let body = overlay_body("root", "world.hotdeal.01", true);
        let receipt = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body).unwrap();
        assert!(!receipt.deduped);
        assert_eq!(receipt.namespaced_kind, "porting.coverage");
        assert!(receipt.location.starts_with("kind=porting.coverage/root__world.hotdeal.01__"), "got {}", receipt.location);

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty(), "unexpected violations: {violations:?}");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1.0["scenario_id"], "world.hotdeal.01");
    }

    #[test]
    fn write_namespaced_byte_identical_resubmission_dedupes_to_same_path_never_erroring() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let envelope_at = Utc::now().to_rfc3339();
        let body = || {
            RawRecord(serde_json::json!({
                "schema": 1, "kind": "porting.coverage", "at": envelope_at,
                "actor": {"agent_id": "porting-sync", "role": "implementer"},
                "project_id": "root", "scenario_id": "world.hotdeal.01", "covered": true,
            }))
        };
        let first = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap();
        assert!(!first.deduped);
        let second = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap();
        assert!(second.deduped, "byte-identical resubmission must dedupe, never a rejected-duplicate error");
        assert_eq!(first.location, second.location);
        assert_eq!(first.digest, second.digest);

        let (records, _) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert_eq!(records.len(), 1, "a dedup no-op must never create a second distinct file");
    }

    #[test]
    fn write_namespaced_a_tampered_existing_path_with_different_content_is_rejected_not_deduped() {
        // F2: the resolved path already exists, but its on-disk bytes
        // are DIFFERENT from the incoming body -- a 12-hex digest
        // collision or a hand-tampered file. This must never be
        // silently reported as `deduped: true` over different content.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let envelope_at = Utc::now().to_rfc3339();
        let body = || {
            RawRecord(serde_json::json!({
                "schema": 1, "kind": "porting.coverage", "at": envelope_at,
                "actor": {"agent_id": "porting-sync", "role": "implementer"},
                "project_id": "root", "scenario_id": "world.hotdeal.01", "covered": true,
            }))
        };
        let receipt = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap();
        assert!(!receipt.deduped);

        // Hand-tamper the file at the exact resolved path: same
        // location, deliberately different content.
        std::fs::write(dir.path().join(&receipt.location), serde_json::json!({"tampered": true}).to_string()).unwrap();

        // Resubmitting the SAME logical body resolves to the SAME
        // digest-suffixed path (`body()` is deterministic) -- an
        // identical resubmission would normally dedupe, but the file
        // now sitting there is NOT what it claims to be.
        let err = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "expected a loud Layout rejection, got {err:?}");
    }

    #[test]
    fn write_namespaced_a_same_value_but_non_canonical_bytes_file_is_rejected_not_deduped() {
        // F2 (byte-level, not merely semantic): a file at the resolved
        // path holding the SAME logical JSON Value but NON-canonical bytes
        // (compact rather than the sorted-key pretty form the write emits)
        // is not what `write_namespaced` would have written, so dedup must
        // reject it loudly rather than report `deduped: true` over bytes it
        // did not author. Guards the corpus invariant that every namespaced
        // file is exactly canon's canonical serialization.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let envelope_at = Utc::now().to_rfc3339();
        let body = || {
            RawRecord(serde_json::json!({
                "schema": 1, "kind": "porting.coverage", "at": envelope_at,
                "actor": {"agent_id": "porting-sync", "role": "implementer"},
                "project_id": "root", "scenario_id": "world.hotdeal.01", "covered": true,
            }))
        };
        let receipt = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap();
        assert!(!receipt.deduped);

        // Overwrite with the SAME Value serialized COMPACT (to_string) --
        // parses back to an equal Value, but differs byte-for-byte from the
        // sorted-key pretty form on disk.
        let compact = serde_json::to_string(&body().0).unwrap();
        let pretty = serde_json::to_vec_pretty(&body().0).unwrap();
        assert_ne!(compact.as_bytes(), pretty.as_slice(), "the two serializations must actually differ");
        std::fs::write(dir.path().join(&receipt.location), &compact).unwrap();

        let err = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body()).unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "a same-Value but non-canonical-bytes file must be a loud Layout rejection, got {err:?}");
    }

    #[test]
    fn write_namespaced_logically_different_body_same_join_key_appends_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let first_body = overlay_body("root", "world.hotdeal.01", false);
        let second_body = overlay_body("root", "world.hotdeal.01", true); // covered flips false -> true
        let first = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", first_body).unwrap();
        let second = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", second_body).unwrap();
        assert!(!second.deduped, "a logically different body must be a genuine append, not a dedup");
        assert_ne!(first.location, second.location, "different content must resolve to a different digest-suffixed path");

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty());
        assert_eq!(records.len(), 2, "the first record must never be deleted or overwritten by the second");
        assert!(dir.path().join(&first.location).exists(), "first record's file must still exist unchanged");
    }

    #[test]
    fn write_namespaced_rejects_namespaced_kind_colliding_with_a_core_recordkind() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let err = tier.write_namespaced("scenario", "x", RawRecord(serde_json::json!({}))).unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "expected a Layout rejection, got {err:?}");
        assert!(!dir.path().join("kind=scenario").exists(), "a core kind directory must never be touched by a namespaced write");
    }

    #[test]
    fn write_namespaced_rejects_a_namespaced_kind_failing_the_two_token_kebab_grammar() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        for bad in ["porting/coverage", "porting", "porting.coverage.extra", "Porting.Coverage", "porting.cov erage", "porting..coverage"] {
            let err = tier.write_namespaced(bad, "x", RawRecord(serde_json::json!({"x": "x"}))).unwrap_err();
            assert!(matches!(err, StoreError::Layout(_)), "expected `{bad}` rejected as Layout, got {err:?}");
        }
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "no directory must be created for any rejected namespaced_kind");
    }

    #[test]
    fn write_namespaced_rejects_a_natural_key_containing_a_path_traversal_before_touching_disk() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        for bad_key in ["../../etc/passwd", "a/b", "a\\b", "..", ".hidden", "/abs"] {
            let err = tier.write_namespaced("porting.coverage", bad_key, overlay_body("root", "world.hotdeal.01", true)).unwrap_err();
            assert!(matches!(err, StoreError::Layout(_)), "expected `{bad_key}` rejected as Layout, got {err:?}");
        }
        assert!(!dir.path().join("kind=porting.coverage").exists(), "no path may be constructed for a rejected natural_key");
    }

    #[test]
    fn write_namespaced_rejects_a_natural_key_disagreeing_with_the_bodys_join_key_values() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        // body carries scenario_id "world.hotdeal.99"; natural_key names ".01" instead.
        let body = overlay_body("root", "world.hotdeal.99", true);
        let err = tier.write_namespaced("porting.coverage", "root__world.hotdeal.01", body).unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "expected a Layout rejection, got {err:?}");
        assert!(!dir.path().join("kind=porting.coverage").exists());
    }

    #[test]
    fn write_namespaced_rejects_a_body_kind_disagreeing_with_the_namespaced_kind_argument() {
        // F1 store-layer defense in depth: body's own top-level `kind`
        // must equal the `namespaced_kind` argument -- an overlay can
        // never be written under `kind=<x>/` while its body claims a
        // different kind.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let body = overlay_body("root", "world.hotdeal.01", true); // body kind is "porting.coverage"
        let err = tier.write_namespaced("other.coverage", "root__world.hotdeal.01", body).unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "expected a Layout rejection, got {err:?}");
        assert!(!dir.path().join("kind=other.coverage").exists(), "no path may be constructed for a body/namespaced_kind mismatch");
    }

    #[test]
    fn scan_namespaced_kind_of_an_absent_directory_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(records.is_empty());
        assert!(violations.is_empty());
    }

    #[test]
    fn scan_namespaced_kind_rejects_namespaced_kind_colliding_with_a_core_recordkind() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let err = tier.scan_namespaced_kind("scenario").unwrap_err();
        assert!(matches!(err, StoreError::Layout(_)), "expected a Layout rejection, got {err:?}");
    }

    #[test]
    fn scan_namespaced_kind_skips_malformed_json_and_digest_mismatched_files() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let kind_dir = dir.path().join("kind=porting.coverage");
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("not-json__abcdef123456.json"), "{not valid json").unwrap();
        // Well-formed JSON, but the filename's digest suffix disagrees
        // with the file's own content -- a planted layout violation, not
        // just a malformed-JSON one.
        std::fs::write(
            kind_dir.join("root__world.hotdeal.02__000000000000.json"),
            serde_json::json!({
                "schema": 1, "kind": "porting.coverage", "at": Utc::now().to_rfc3339(),
                "actor": {"agent_id": "a", "role": "implementer"},
                "project_id": "root", "scenario_id": "world.hotdeal.02", "covered": true,
            })
            .to_string(),
        )
        .unwrap();

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(records.is_empty());
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn scan_namespaced_kind_excludes_a_planted_file_whose_body_kind_disagrees_with_its_directory() {
        // F1 store-layer defense in depth, scan half: a file living
        // under `kind=porting.coverage/` whose own body `kind` claims
        // a DIFFERENT overlay identity must be excluded/violation-
        // flagged, never returned as an active overlay record.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let kind_dir = dir.path().join("kind=porting.coverage");
        std::fs::create_dir_all(&kind_dir).unwrap();
        let planted = serde_json::json!({
            "schema": 1, "kind": "other.coverage", "at": Utc::now().to_rfc3339(),
            "actor": {"agent_id": "a", "role": "implementer"},
            "project_id": "root", "scenario_id": "world.hotdeal.03", "covered": true,
        });
        let digest = crate::partition::content_digest12(&planted);
        std::fs::write(kind_dir.join(format!("root__world.hotdeal.03__{digest}.json")), planted.to_string()).unwrap();

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(records.is_empty(), "a body/directory kind mismatch must never be returned as an active record");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::Malformed);
    }

    #[test]
    fn scan_namespaced_kind_excludes_a_nested_path_and_a_non_json_extension_file() {
        // F3: the canonical overlay path shape is FLAT
        // (`kind=<x>/<file>.json`) -- a nested subdirectory or a
        // non-`.json` extension is a layout violation, never returned,
        // even when the file's own content is otherwise well-formed
        // and its digest suffix is self-consistent.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        let kind_dir = dir.path().join("kind=porting.coverage");
        let nested_dir = kind_dir.join("attic");
        std::fs::create_dir_all(&nested_dir).unwrap();

        let body_a = overlay_body("root", "world.hotdeal.04", true).0;
        let digest_a = crate::partition::content_digest12(&body_a);
        std::fs::write(nested_dir.join(format!("root__world.hotdeal.04__{digest_a}.json")), body_a.to_string()).unwrap();

        let body_b = overlay_body("root", "world.hotdeal.05", true).0;
        let digest_b = crate::partition::content_digest12(&body_b);
        std::fs::write(kind_dir.join(format!("root__world.hotdeal.05__{digest_b}.txt")), body_b.to_string()).unwrap();

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(records.is_empty(), "neither a nested path nor a non-.json file may be returned as an active record");
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn scan_namespaced_kind_a_canonical_flat_json_file_still_round_trips() {
        // F3's positive twin: a genuinely canonical flat `.json` file
        // must still round-trip, unaffected by the new layout checks.
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path());
        tier.write_namespaced("porting.coverage", "root__world.hotdeal.06", overlay_body("root", "world.hotdeal.06", true)).unwrap();

        let (records, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty(), "{violations:?}");
        assert_eq!(records.len(), 1);
    }
}
