//! This repo's OWN root `canon.yaml` (canon dogfoods itself) parses
//! cleanly as a `TierPolicy` — every one of the twelve record kinds
//! routes somewhere, `aging` resolves both entries — AND S1's
//! `handoff_templates:` key in the SAME file still parses independently
//! (tier-policy spec's "S1 owns only this narrow slice" convention:
//! two specs' sections coexist in one file without either's parser
//! choking on the other's keys).

use canon_model::envelope::RecordKind;
use canon_model::handoff::{DomainId, GihoekTemplate, TemplateRegistry};
use canon_store::policy::TierPolicy;

fn root_canon_yaml() -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../canon.yaml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

#[test]
fn every_record_kind_routes_somewhere_in_the_shipped_canon_yaml() {
    let yaml = root_canon_yaml();
    let policy = TierPolicy::from_yaml(&yaml).expect("this repo's canon.yaml must parse as a valid TierPolicy");

    for kind in RecordKind::ALL {
        policy.tier_for(kind).unwrap_or_else(|e| panic!("{}: {e}", kind.as_str()));
    }
    assert_eq!(policy.aging.len(), 2, "handoff + event, per D3's worked example");
}

#[test]
fn s1_handoff_templates_and_s2_tier_policy_coexist_in_one_canon_yaml() {
    let yaml = root_canon_yaml();

    // S2's parser.
    TierPolicy::from_yaml(&yaml).expect("S2's TierPolicy parse must succeed against the real file");

    // S1's parser, unaffected by S2's `tiers`/`routing`/`aging` keys.
    let registry = TemplateRegistry::from_manifest(&yaml, vec![Box::new(GihoekTemplate)])
        .expect("S1's HandoffTemplatesManifest parse must succeed against the real file");
    assert!(registry.is_registered(&DomainId::parse("기획").unwrap()));
}
