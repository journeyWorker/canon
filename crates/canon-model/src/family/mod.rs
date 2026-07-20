//! The artifact-family schema registry (S11 `artifact-family-schema`
//! spec, design D1): canon-model's format-authority layer over the
//! whole ledger/divergence/inventory/features/policy family, layered
//! ALONGSIDE (not folded into) the closed twelve [`crate::envelope::RecordKind`]
//! kinds — the donor vocabulary (`run`/`drill`/`review`/`clear`/
//! `code-review`/`design-review` ledger kinds, `manifest`/`review`/
//! `remediation` divergence events, `inventory`/`inventory-lock`/
//! `feature`/`policy`) has more distinct on-disk `kind`/`type` wire
//! values than RecordKind's twelve, so it earns its own kind identity
//! ([`FamilyKind`]) rather than overloading `RecordKind::Run`/`Review`/
//! `Divergence` (canon's OWN internal operational records) to also mean
//! six different donor ledger shapes.
//!
//! [`FamilyEnvelope`] is the same `{schema, kind, at, actor}` shape as
//! [`crate::envelope::Envelope`] (never a bare `by` string, same
//! discipline), generic over each family's own closed kind enum so the
//! envelope itself is defined exactly once.
//!
//! [`LayoutDescriptor`]/[`layout_problem`] (task 1.2/1.3) generalize
//! `tools/parity.py::_ledger_layout_problem` to every registered kind —
//! see [`LayoutDescriptor`]'s own docs for the declarative shape.

pub mod divergence;
pub mod feature;
pub mod inventory;
pub mod ledger;
pub mod policy;
pub mod refs;

use std::path::Path;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::Actor;
pub use ledger::LedgerKind;

/// The artifact family's own kind identity — a strict superset of
/// on-disk `kind`/root-directory identity beyond `RecordKind`'s closed
/// twelve. A thirteenth-and-beyond wire value here is exactly as closed
/// as `RecordKind` (adding one is a reviewed `canon-model` change, never
/// an open string) — see [`FamilyKind::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FamilyKind {
    /// One of the six ledger sub-kinds (`run`/`drill`/`review`/`clear`/
    /// `code-review`/`design-review`) — see [`LedgerKind`].
    Ledger(LedgerKind),
    /// `spec/divergences/lane=<lane>/area=<area>/surface=<surface>/*.jsonl`
    /// — ONE family kind at the layout level; each JSONL line is
    /// individually one of [`divergence::DivergenceEvent`]'s three
    /// event shapes (`manifest`/`review`/`remediation`).
    Divergence,
    /// `spec/features/kind=feature/area=<area>/<surface>.feature`.
    Feature,
    /// `spec/inventory/kind=inventory/area=<area>/[surface=<surface>/]<key>.yaml`.
    Inventory,
    /// `spec/inventory/kind=inventory-lock/assets.lock.yaml` — a single
    /// generated-only lockfile, D16 pattern (regenerate + diff-check).
    InventoryLock,
    /// `spec/policy.yaml` — envelope-only upgrade, path unchanged
    /// (design D3 only details inventory/assets.lock moving; policy is
    /// not called out as a layout migration).
    Policy,
}

impl FamilyKind {
    /// Every family kind, ledger's six sub-kinds expanded — the one
    /// iteration point the schema exporter and layout registry both
    /// walk (mirrors [`crate::envelope::RecordKind::ALL`]'s role).
    pub const ALL: [FamilyKind; 11] = [
        FamilyKind::Ledger(LedgerKind::Run),
        FamilyKind::Ledger(LedgerKind::Drill),
        FamilyKind::Ledger(LedgerKind::Review),
        FamilyKind::Ledger(LedgerKind::Clear),
        FamilyKind::Ledger(LedgerKind::CodeReview),
        FamilyKind::Ledger(LedgerKind::DesignReview),
        FamilyKind::Divergence,
        FamilyKind::Feature,
        FamilyKind::Inventory,
        FamilyKind::InventoryLock,
        FamilyKind::Policy,
    ];

    /// The wire/directory kind string (`kind=<this>/`, or the divergence
    /// event's own `type` value with no `kind=` segment on disk).
    pub fn as_str(self) -> &'static str {
        match self {
            FamilyKind::Ledger(k) => k.as_str(),
            FamilyKind::Divergence => "divergence",
            FamilyKind::Feature => "feature",
            FamilyKind::Inventory => "inventory",
            FamilyKind::InventoryLock => "inventory-lock",
            FamilyKind::Policy => "policy",
        }
    }
}

/// The shared `{schema, kind, at, actor}` envelope (S11 design "Schema
/// envelopes ... on every artifact, including YAML"), generic over each
/// family's own closed kind enum ([`LedgerKind`], or a unit struct for
/// families with exactly one kind — see [`feature::FeatureKind`] etc.)
/// so this one definition serves every family member. Composed via
/// `#[serde(flatten)]`, exactly like [`crate::envelope::Envelope`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FamilyEnvelope<K> {
    pub schema: u32,
    pub kind: K,
    pub at: DateTime<Utc>,
    pub actor: Actor,
}

impl<K> FamilyEnvelope<K> {
    pub fn new(schema: u32, kind: K, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema, kind, at, actor }
    }
}

/// A leaf (final path component, possibly nested one directory deep —
/// see [`LeafGrammar::InventoryYaml`]) filename grammar, distinguishing
/// exactly the shapes S11 design D1 catalogs. Each variant's
/// [`LeafGrammar::check`] is a structural, hand-rolled predicate
/// (mirrors `tools/parity.py::_RUN_FN_RE`/the donor's exact-basename
/// check) — no `regex` dependency, matching `crate::ids`'s existing
/// hand-written-grammar-validator convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafGrammar {
    /// `<scenario_id>.json` — the filename MUST equal the record's own
    /// resolved `scenario_id` plus `.json`, exactly (ledger review-family).
    ScenarioIdJson,
    /// `<8-digit-date>T<6-digit-time>-<lane>-<sha-prefix>-<digest6>.json`
    /// — format-only (no content cross-check: a `run`/`drill` record
    /// covers zero or many scenarios, so nothing in its own content is
    /// the filename's sole source of truth beyond the timestamp/lane it
    /// already carries as fields).
    TimestampedJson,
    /// `<round>-<round>-<sha-prefix>-<rand>.jsonl` (divergences) — the
    /// donor's own leaf grammar, layout UNCHANGED by S11 (design
    /// Non-Goal); format-only.
    DivergenceJsonl,
    /// `<surface>.feature`.
    SurfaceFeature,
    /// `<key>.yaml`, the leaf itself un-constrained beyond the
    /// extension (D3: `<key>` is the donor's own historical basename,
    /// preserved verbatim — only the NEW `kind=`/`area=`/`surface=`
    /// prefix is added).
    InventoryYaml,
    /// One exact, fixed filename (`assets.lock.yaml`, `policy.yaml`).
    Fixed(&'static str),
}

impl LeafGrammar {
    /// Structural check only — does NOT know the record's own content
    /// (callers needing an exact `scenario_id`-equals-leaf check do that
    /// themselves via [`ResolvedPartition::leaf_name`], since only the
    /// caller has parsed content available).
    pub fn matches_shape(self, leaf: &str) -> bool {
        match self {
            LeafGrammar::ScenarioIdJson => leaf.ends_with(".json") && leaf.len() > 5,
            LeafGrammar::TimestampedJson => is_timestamped_json_leaf(leaf),
            LeafGrammar::DivergenceJsonl => is_divergence_jsonl_leaf(leaf),
            LeafGrammar::SurfaceFeature => leaf.ends_with(".feature") && leaf.len() > 8,
            LeafGrammar::InventoryYaml => leaf.ends_with(".yaml") && leaf.len() > 5,
            LeafGrammar::Fixed(name) => leaf == name,
        }
    }
}

/// `<8-digit-date>T<6-digit-time>-<lane>-<hex-sha-prefix>-<6-hex-digest>.json`
/// (`tools/parity.py::_RUN_FN_RE`, ported as a hand-written scan).
fn is_timestamped_json_leaf(leaf: &str) -> bool {
    let Some(stem) = leaf.strip_suffix(".json") else { return false };
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    let [stamp, lane, sha, digest] = parts.as_slice() else { return false };
    let stamp_ok = stamp.len() == 15
        && stamp.as_bytes()[8] == b'T'
        && stamp[..8].chars().all(|c| c.is_ascii_digit())
        && stamp[9..].chars().all(|c| c.is_ascii_digit());
    let lane_ok = !lane.is_empty() && lane.chars().all(|c| c.is_ascii_lowercase());
    let sha_ok = sha.len() >= 6 && sha.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    let digest_ok = digest.len() == 6 && digest.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    stamp_ok && lane_ok && sha_ok && digest_ok
}

/// `<round>-<round>-<hex-sha-prefix>-<rand>.jsonl` — the two `<round>`
/// components are the same integer in every real sample observed
/// (`4-4-…`, `1-1-…`); validated as "both integers", not "both equal",
/// since nothing in the donor's own code asserts equality either.
fn is_divergence_jsonl_leaf(leaf: &str) -> bool {
    let Some(stem) = leaf.strip_suffix(".jsonl") else { return false };
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    let [round_a, round_b, sha, rand] = parts.as_slice() else { return false };
    let is_digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let sha_ok = sha.len() >= 6 && sha.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    let rand_ok = !rand.is_empty() && rand.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    is_digits(round_a) && is_digits(round_b) && sha_ok && rand_ok
}

/// One artifact kind's declared Hive layout (S11 design D1): the
/// on-disk `key=value/` segments it requires, in order, and its leaf
/// filename grammar. `canon-model` registers exactly one of these per
/// [`FamilyKind`] ([`FamilyKind::layout_descriptor`]) — a single
/// declarative table, not per-kind special-casing scattered across
/// callers.
#[derive(Debug, Clone, Copy)]
pub struct LayoutDescriptor {
    pub kind: FamilyKind,
    /// The top-level directory this kind's files live under, relative
    /// to the corpus root (`spec/`) — e.g. `"ledger"`, `"divergences"`,
    /// `"features"`, `"inventory"`. Divergences' root itself is the
    /// kind-discriminator (no `kind=divergence/` segment is added,
    /// design Non-Goal: layout unchanged); every other family gains an
    /// explicit `kind=<value>/` segment directly under its root.
    pub root_dir: &'static str,
    /// The `kind=<value>/` segment directly under `root_dir`, if this
    /// kind uses one. `None` only for [`FamilyKind::Divergence`].
    pub kind_segment: Option<&'static str>,
    /// `key=value/` segments AFTER the optional kind segment, in order
    /// — e.g. `["area"]` for ledger review-family, `["lane", "area",
    /// "surface"]` for divergences, `[]` for ledger run/drill (D1's
    /// explicit zero-partition-key exception).
    pub partition_keys: &'static [&'static str],
    /// An OPTIONAL `key=value/` segment allowed immediately before the
    /// leaf filename, on top of `partition_keys` (S11 design D3:
    /// inventory's `[surface=<surface>/]` segment). Unlike
    /// `partition_keys`, a path is conforming whether it includes this
    /// segment or omits it — the content-resolved value only needs to
    /// match WHEN the path segment is present (see
    /// [`ResolvedPartition::optional_segment`]). `None` for every kind
    /// except [`FamilyKind::Inventory`].
    pub optional_segment_key: Option<&'static str>,
    pub leaf: LeafGrammar,
}

impl FamilyKind {
    pub fn layout_descriptor(self) -> LayoutDescriptor {
        match self {
            FamilyKind::Ledger(LedgerKind::Run) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("run"),
                partition_keys: &[],
                optional_segment_key: None,
                leaf: LeafGrammar::TimestampedJson,
            },
            FamilyKind::Ledger(LedgerKind::Drill) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("drill"),
                partition_keys: &[],
                optional_segment_key: None,
                leaf: LeafGrammar::TimestampedJson,
            },
            FamilyKind::Ledger(LedgerKind::Review) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("review"),
                partition_keys: &["area"],
                optional_segment_key: None,
                leaf: LeafGrammar::ScenarioIdJson,
            },
            FamilyKind::Ledger(LedgerKind::Clear) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("clear"),
                partition_keys: &["area"],
                optional_segment_key: None,
                leaf: LeafGrammar::ScenarioIdJson,
            },
            FamilyKind::Ledger(LedgerKind::CodeReview) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("code-review"),
                partition_keys: &["area"],
                optional_segment_key: None,
                leaf: LeafGrammar::ScenarioIdJson,
            },
            FamilyKind::Ledger(LedgerKind::DesignReview) => LayoutDescriptor {
                kind: self,
                root_dir: "ledger",
                kind_segment: Some("design-review"),
                partition_keys: &["area"],
                optional_segment_key: None,
                leaf: LeafGrammar::ScenarioIdJson,
            },
            FamilyKind::Divergence => LayoutDescriptor {
                kind: self,
                root_dir: "divergences",
                kind_segment: None,
                partition_keys: &["lane", "area", "surface"],
                optional_segment_key: None,
                leaf: LeafGrammar::DivergenceJsonl,
            },
            FamilyKind::Feature => LayoutDescriptor {
                kind: self,
                root_dir: "features",
                kind_segment: Some("feature"),
                partition_keys: &["area"],
                optional_segment_key: None,
                leaf: LeafGrammar::SurfaceFeature,
            },
            FamilyKind::Inventory => LayoutDescriptor {
                kind: self,
                root_dir: "inventory",
                kind_segment: Some("inventory"),
                partition_keys: &["area"],
                optional_segment_key: Some("surface"),
                leaf: LeafGrammar::InventoryYaml,
            },
            FamilyKind::InventoryLock => LayoutDescriptor {
                kind: self,
                root_dir: "inventory",
                kind_segment: Some("inventory-lock"),
                partition_keys: &[],
                optional_segment_key: None,
                leaf: LeafGrammar::Fixed("assets.lock.yaml"),
            },
            FamilyKind::Policy => LayoutDescriptor {
                kind: self,
                root_dir: "",
                kind_segment: None,
                partition_keys: &[],
                optional_segment_key: None,
                leaf: LeafGrammar::Fixed("policy.yaml"),
            },
        }
    }
}

/// A record's OWN partition-key values, resolved from its content by a
/// kind-specific (but pure, content-only) rule — never from the
/// directory it was found in (the `_area_of` gotcha, generalized to
/// every partition key of every kind). `inventory`'s resolver may
/// legitimately fail to agree across entries (D8 `ambiguous-partition`).
#[derive(Debug, Clone, Default)]
pub struct ResolvedPartition {
    /// `(key, value)` pairs, in the SAME order as the descriptor's
    /// `partition_keys` — `layout_problem` zips them positionally.
    pub values: Vec<(&'static str, String)>,
    /// The content-derived exact leaf basename, when the leaf grammar
    /// requires one ([`LeafGrammar::ScenarioIdJson`]'s `<scenario_id>.json`).
    pub leaf_name: Option<String>,
    /// The content-derived value for the descriptor's
    /// `optional_segment_key`, if this kind resolves one — e.g.
    /// inventory's `surface`. `layout_problem` accepts the path WITH
    /// or WITHOUT this segment; when present, its value must match.
    /// `None` for every kind whose descriptor has no
    /// `optional_segment_key` (also `None` when a kind that HAS one
    /// still can't resolve a value — treated the same as "omitted").
    pub optional_segment: Option<String>,
}

/// Why a file's actual path disagrees with its own content's declared
/// layout (S11 design D1, generalizing `_ledger_layout_problem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutViolation {
    pub expected: String,
    pub actual: String,
    pub detail: String,
}

/// The generalized `_ledger_layout_problem`: does `relative_path` (a
/// corpus-root-relative path) match `descriptor`, given `resolved`'s
/// content-derived partition values? `None` means the layout is
/// conforming. This function only compares SHAPE/VALUES it is handed —
/// deriving `resolved` from a record's own content is each family
/// module's job (`ledger`/`divergence`/`feature`/`inventory`), never
/// this function's, so `layout_problem` stays format-agnostic (JSON,
/// YAML, and Gherkin content all resolve to the same `ResolvedPartition`
/// shape before reaching here).
pub fn layout_problem(descriptor: &LayoutDescriptor, relative_path: &Path, resolved: &ResolvedPartition) -> Option<LayoutViolation> {
    let components: Vec<String> = relative_path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let mut expected_prefix: Vec<String> = Vec::new();
    if !descriptor.root_dir.is_empty() {
        expected_prefix.push(descriptor.root_dir.to_string());
    }
    if let Some(seg) = descriptor.kind_segment {
        expected_prefix.push(format!("kind={seg}"));
    }
    for (key, value) in &resolved.values {
        expected_prefix.push(format!("{key}={value}"));
    }

    // The optional segment is conforming BOTH present and absent — it
    // only becomes a fixed expectation once the descriptor declares the
    // key AND content resolves a value for it.
    let optional_segment = match (descriptor.optional_segment_key, &resolved.optional_segment) {
        (Some(key), Some(value)) => Some(format!("{key}={value}")),
        _ => None,
    };

    let expected_str = || {
        let mut s = expected_prefix.join("/");
        if !s.is_empty() {
            s.push('/');
        }
        if let Some(key) = descriptor.optional_segment_key {
            s.push_str(&format!("[{key}=<{key}>/]"));
        }
        match resolved.leaf_name.as_deref() {
            Some(leaf) => format!("{s}{leaf}"),
            None => format!("{s}<{}>", leaf_grammar_label(descriptor.leaf)),
        }
    };

    if components.len() < expected_prefix.len() + 1 {
        return Some(LayoutViolation {
            expected: expected_str(),
            actual: relative_path.display().to_string(),
            detail: format!(
                "expected at least {} path segment(s) before the leaf filename, found {}",
                expected_prefix.len(),
                components.len().saturating_sub(1)
            ),
        });
    }
    for (i, want) in expected_prefix.iter().enumerate() {
        if &components[i] != want {
            return Some(LayoutViolation {
                expected: expected_str(),
                actual: relative_path.display().to_string(),
                detail: format!("path segment {i} is `{}`, expected `{want}`", components[i]),
            });
        }
    }

    // Everything after the fixed prefix: exactly the leaf (no optional
    // segment in the path), or — only when this kind declares one — the
    // optional segment followed by the leaf.
    let remainder = &components[expected_prefix.len()..];
    let leaf: &str = match remainder {
        [leaf] => leaf,
        [actual_segment, leaf] if optional_segment.is_some() => {
            let expected_segment = optional_segment.as_deref().expect("checked is_some above");
            if actual_segment != expected_segment {
                return Some(LayoutViolation {
                    expected: expected_str(),
                    actual: relative_path.display().to_string(),
                    detail: format!(
                        "optional segment is `{actual_segment}`, expected `{expected_segment}` (derived from the record's own content)"
                    ),
                });
            }
            leaf
        }
        _ => {
            let max_segments = expected_prefix.len() + if optional_segment.is_some() { 2 } else { 1 };
            return Some(LayoutViolation {
                expected: expected_str(),
                actual: relative_path.display().to_string(),
                detail: format!(
                    "expected {} path segment(s) before the leaf filename (up to {max_segments} total with the optional `{}=` segment), found {}",
                    expected_prefix.len(),
                    descriptor.optional_segment_key.unwrap_or(""),
                    components.len().saturating_sub(1)
                ),
            });
        }
    };
    if let Some(exact) = &resolved.leaf_name {
        if leaf != exact {
            return Some(LayoutViolation {
                expected: expected_str(),
                actual: relative_path.display().to_string(),
                detail: format!("leaf filename is `{leaf}`, expected `{exact}` (derived from the record's own content)"),
            });
        }
    } else if !descriptor.leaf.matches_shape(leaf) {
        return Some(LayoutViolation {
            expected: expected_str(),
            actual: relative_path.display().to_string(),
            detail: format!("leaf filename `{leaf}` does not match the `{}` grammar", leaf_grammar_label(descriptor.leaf)),
        });
    }
    None
}

fn leaf_grammar_label(leaf: LeafGrammar) -> &'static str {
    match leaf {
        LeafGrammar::ScenarioIdJson => "<scenario_id>.json",
        LeafGrammar::TimestampedJson => "<stamp>-<lane>-<sha>-<digest>.json",
        LeafGrammar::DivergenceJsonl => "<round>-<round>-<sha>-<rand>.jsonl",
        LeafGrammar::SurfaceFeature => "<surface>.feature",
        LeafGrammar::InventoryYaml => "<key>.yaml",
        LeafGrammar::Fixed(name) => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_eleven_family_kinds_present_exactly_once() {
        assert_eq!(FamilyKind::ALL.len(), 11);
        let mut seen = std::collections::HashSet::new();
        for kind in FamilyKind::ALL {
            assert!(seen.insert(kind.as_str()), "{} listed twice", kind.as_str());
        }
    }

    #[test]
    fn run_drill_zero_partition_keys_is_not_flagged_missing_area() {
        let descriptor = FamilyKind::Ledger(LedgerKind::Run).layout_descriptor();
        let resolved = ResolvedPartition::default();
        let path = Path::new("ledger/kind=run/20260707T155227-unit-2745ca4c8-bbb64d.json");
        assert_eq!(layout_problem(&descriptor, path, &resolved), None);
    }

    #[test]
    fn review_area_scoped_leaf_must_equal_scenario_id() {
        let descriptor = FamilyKind::Ledger(LedgerKind::Review).layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("area", "idolive".to_string())],
            leaf_name: Some("idolive.hub.25.json".to_string()),
            optional_segment: None,
        };
        let good = Path::new("ledger/kind=review/area=idolive/idolive.hub.25.json");
        assert_eq!(layout_problem(&descriptor, good, &resolved), None);

        let wrong_leaf = Path::new("ledger/kind=review/area=idolive/wrong-name.json");
        assert!(layout_problem(&descriptor, wrong_leaf, &resolved).is_some());

        let wrong_area = Path::new("ledger/kind=review/area=world/idolive.hub.25.json");
        assert!(layout_problem(&descriptor, wrong_area, &resolved).is_some());
    }

    #[test]
    fn review_leaf_with_an_unexpected_extra_segment_is_rejected() {
        // A kind with no `optional_segment_key` (review) must still
        // reject an extra path segment beyond its declared partition —
        // the optional-trailing-segment allowance is Inventory-only.
        let descriptor = FamilyKind::Ledger(LedgerKind::Review).layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("area", "idolive".to_string())],
            leaf_name: Some("idolive.hub.25.json".to_string()),
            optional_segment: None,
        };
        let extra_segment = Path::new("ledger/kind=review/area=idolive/surface=hub/idolive.hub.25.json");
        assert!(layout_problem(&descriptor, extra_segment, &resolved).is_some());
    }

    #[test]
    fn feature_flat_pre_migration_path_is_a_violation() {
        let descriptor = FamilyKind::Feature.layout_descriptor();
        let resolved = ResolvedPartition { values: vec![("area", "idolive".to_string())], leaf_name: None, optional_segment: None };
        let flat = Path::new("features/idolive/idolive-hub.feature");
        assert!(layout_problem(&descriptor, flat, &resolved).is_some());
        let migrated = Path::new("features/kind=feature/area=idolive/idolive-hub.feature");
        assert_eq!(layout_problem(&descriptor, migrated, &resolved), None);
    }

    #[test]
    fn divergence_three_key_partition_matches_real_shape() {
        let descriptor = FamilyKind::Divergence.layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("lane", "code".to_string()), ("area", "idolive".to_string()), ("surface", "idolive-hub".to_string())],
            leaf_name: None,
            optional_segment: None,
        };
        let path = Path::new("divergences/lane=code/area=idolive/surface=idolive-hub/4-4-6e00ea28-e16b3d.jsonl");
        assert_eq!(layout_problem(&descriptor, path, &resolved), None);
    }

    #[test]
    fn inventory_accepts_the_optional_surface_segment_when_content_agrees() {
        let descriptor = FamilyKind::Inventory.layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("area", "idolive".to_string())],
            leaf_name: None,
            optional_segment: Some("hub".to_string()),
        };
        let with_surface = Path::new("inventory/kind=inventory/area=idolive/surface=hub/idolive-hub.yaml");
        assert_eq!(layout_problem(&descriptor, with_surface, &resolved), None);

        let without_surface = Path::new("inventory/kind=inventory/area=idolive/idolive-hub.yaml");
        assert_eq!(layout_problem(&descriptor, without_surface, &resolved), None);
    }

    #[test]
    fn inventory_rejects_a_surface_segment_disagreeing_with_content() {
        let descriptor = FamilyKind::Inventory.layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("area", "idolive".to_string())],
            leaf_name: None,
            optional_segment: Some("hub".to_string()),
        };
        let wrong_surface = Path::new("inventory/kind=inventory/area=idolive/surface=wrong/idolive-hub.yaml");
        assert!(layout_problem(&descriptor, wrong_surface, &resolved).is_some());
    }

    #[test]
    fn inventory_rejects_a_genuinely_malformed_extra_segment() {
        // Three segments after `area=` (neither `surface=<content-value>/<key>.yaml`
        // nor a bare `<key>.yaml`) is still a layout violation — the
        // optional segment is bounded, not an open-ended extra depth.
        let descriptor = FamilyKind::Inventory.layout_descriptor();
        let resolved = ResolvedPartition {
            values: vec![("area", "idolive".to_string())],
            leaf_name: None,
            optional_segment: Some("hub".to_string()),
        };
        let too_deep = Path::new("inventory/kind=inventory/area=idolive/surface=hub/extra/idolive-hub.yaml");
        assert!(layout_problem(&descriptor, too_deep, &resolved).is_some());
    }
}
