//! Round-trip property test (design.md §8 pattern, task 4.6): "for a
//! generated corpus of valid atoms, compile → decompile → compile is
//! idempotent." Unlike `compile::tests::compile_decompile_compile_round_trip_
//! is_idempotent` (two hand-written example atoms), this generates a wide
//! corpus of valid `task` atoms (varying `desc`/`owner`/`status`/
//! `evidence.kind`/`evidence.ref`) via `proptest` and asserts the SAME
//! property holds for every one of them.

use std::collections::BTreeMap;

use canon_model::{Actor, Envelope, RecordKind, RoleId, TaskStatus};
use canon_vocab::manifest::schema::DirectiveDecl;
use canon_vocab::manifest::snapshot::CapabilitySnapshot;
use canon_vocab::manifest::types::{AttrDecl, Type};
use canon_vocab::{compile_task, decompile_task, AtomRecord};
use proptest::prelude::*;

const EVIDENCE_KINDS: [&str; 3] = ["test-run", "manual-review", "ci-log"];

fn snapshot() -> CapabilitySnapshot {
    let mut snap = CapabilitySnapshot::default();
    snap.directives.insert(
        "task".to_string(),
        DirectiveDecl {
            name: "task".into(),
            attrs: vec![
                AttrDecl { name: "desc".into(), required: true, ty: Type::Str, default: None },
                AttrDecl { name: "owner".into(), required: false, ty: Type::Str, default: None },
                AttrDecl { name: "status".into(), required: true, ty: Type::Domain("task-status".into()), default: None },
                AttrDecl { name: "evidence".into(), required: true, ty: Type::Evidence, default: None },
            ],
        },
    );
    snap.enums.insert("task-status".to_string(), vec!["open".into(), "done".into()]);
    snap.evidence_kinds = EVIDENCE_KINDS.iter().map(|s| s.to_string()).collect();
    snap
}

fn envelope() -> Envelope {
    Envelope::new(1, RecordKind::Task, chrono::Utc::now(), Actor::new("proptest-agent", RoleId::parse("implementer").unwrap()))
}

/// A valid task atom, generated from independently-arbitrary field values —
/// every generated instance MUST pass the checker (non-empty printable
/// strings, `status` from the declared domain, `evidence.kind` from the
/// policy-resolved domain).
fn valid_task_atom() -> impl Strategy<Value = AtomRecord> {
    let desc = "[a-zA-Z0-9 ,.'-]{1,80}";
    let owner = prop::option::of("[a-z]{1,20}");
    let status = prop_oneof![Just("open".to_string()), Just("done".to_string())];
    let kind = prop_oneof![Just(EVIDENCE_KINDS[0].to_string()), Just(EVIDENCE_KINDS[1].to_string()), Just(EVIDENCE_KINDS[2].to_string())];
    let evidence_ref = "[a-zA-Z0-9:/._-]{1,60}";
    let n = 1u32..999u32;

    (desc, owner, status, kind, evidence_ref, n).prop_map(|(desc, owner, status, kind, evidence_ref, n)| {
        let mut attrs: BTreeMap<String, serde_yaml::Value> = BTreeMap::new();
        attrs.insert("desc".to_string(), serde_yaml::Value::String(desc));
        if let Some(owner) = owner {
            attrs.insert("owner".to_string(), serde_yaml::Value::String(owner));
        }
        attrs.insert("status".to_string(), serde_yaml::Value::String(status));
        let mut evidence = BTreeMap::new();
        evidence.insert("kind".to_string(), serde_yaml::Value::String(kind));
        evidence.insert("ref".to_string(), serde_yaml::Value::String(evidence_ref));
        attrs.insert("evidence".to_string(), serde_yaml::to_value(evidence).unwrap());
        AtomRecord { id: format!("s10-typed-authoring-vocabulary#{n}"), tag: "task".to_string(), attrs }
    })
}

proptest! {
    #[test]
    fn compile_decompile_compile_is_idempotent_over_a_generated_corpus(atom in valid_task_atom()) {
        let snap = snapshot();

        let task1 = compile_task(&atom, &snap, envelope()).expect("every generated atom is valid");
        let decompiled = decompile_task(&task1).expect("a canon-vocab-compiled task always decompiles");

        prop_assert_eq!(&decompiled.id, &atom.id);
        prop_assert_eq!(&decompiled.tag, &atom.tag);
        prop_assert_eq!(&decompiled.attrs, &atom.attrs);

        // The decompiled atom itself passes validation (task 4.3's contract).
        let diags = canon_vocab::checker::check_directive(&decompiled.tag, &decompiled.attrs, &snap, &decompiled.id);
        prop_assert!(diags.is_empty(), "decompiled atom diagnostics: {diags:?}");

        let task2 = compile_task(&decompiled, &snap, envelope()).expect("recompiles");
        prop_assert_eq!(task1.task_id, task2.task_id);
        prop_assert_eq!(task1.title, task2.title);
        prop_assert_eq!(task1.status == TaskStatus::Done, task2.status == TaskStatus::Done);
        prop_assert_eq!(task1.evidence_note, task2.evidence_note);
    }
}
