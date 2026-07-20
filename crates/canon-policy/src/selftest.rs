//! [`selftest`]: canon-policy's own shared-contract selftest entry point
//! (Wave-2 `canon selftest` aggregator, per-crate registration, S12
//! task "Plus") — wraps this crate's three CEL fixture flows
//! (`tests/equivalence.rs`, `tests/rejection.rs`, `tests/determinism.rs`)
//! as the SAME checks those files exercise; each of those files is now
//! a thin `#[test]` wrapper over this function, never a second,
//! independently-maintained copy of the same fixture logic (mirrors
//! `canon-vocab/src/selftest.rs`'s and `canon-store/src/lib.rs::
//! selftest`'s own precedent for this exact shape).
//!
//! Purely in-memory: every fixture here is a synthetic CEL expression
//! plus a hand-built `serde_json::Value` record, never a filesystem or
//! network read — side-effect-free against the real repo by
//! construction, no rebindable root needed.

use canon_model::RecordKind;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

use crate::{bindings_for, compile, evaluate, Diagnostic, EvalBudget, PolicyValue, SchemaRegistry};

/// `Ok(n)` reports how many of this crate's independent CEL fixture
/// flows passed (3 today: equivalence, rejection, determinism).
/// `Err(_)` carries one human-readable line per failing flow, EVERY
/// failure collected (never short-circuits on the first one) — never
/// panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let mut passed = 0usize;
    let mut failures = Vec::new();

    match check_equivalence() {
        Ok(()) => passed += 1,
        Err(e) => failures.push(e),
    }
    match check_rejection() {
        Ok(()) => passed += 1,
        Err(e) => failures.push(e),
    }
    match check_determinism() {
        Ok(()) => passed += 1,
        Err(e) => failures.push(e),
    }

    if failures.is_empty() {
        Ok(passed)
    } else {
        Err(failures)
    }
}

// ---- equivalence (mirrors the former `tests/equivalence.rs` body,
// task 6.1 / spec "A CEL policy.yaml derives the same required cells as
// an equivalent static-map fixture") ----

/// One CEL-expressed required-cell rule: `cell` is required when `cel`
/// evaluates to `true`.
struct CelRule {
    cell: &'static str,
    cel: &'static str,
}

const CEL_RULES: &[CelRule] = &[
    CelRule { cell: "evidence-review", cel: "record.status == 'done' && age_days(record.at) > 7" },
    CelRule { cell: "stale-flag", cel: "record.status == 'open' && age_days(record.at) > 30" },
];

/// The identical two rules, expressed as a static Rust map/`match` over
/// the same fields — no CEL involved at all.
fn static_map_required_cells(status: &str, age_days: i64) -> Vec<&'static str> {
    let mut cells = Vec::new();
    if status == "done" && age_days > 7 {
        cells.push("evidence-review");
    }
    if status == "open" && age_days > 30 {
        cells.push("stale-flag");
    }
    cells
}

fn cel_required_cells(bindings: &crate::BindingSet, record: &Value, now: DateTime<Utc>) -> Result<Vec<&'static str>, String> {
    let mut cells = Vec::new();
    for rule in CEL_RULES {
        let compiled = compile(rule.cel, bindings).map_err(|e| format!("equivalence: fixture rule `{}` failed write-time validation: {e:?}", rule.cel))?;
        let result = evaluate(&compiled, record, now, EvalBudget::default()).map_err(|e| format!("equivalence: fixture rule `{}` failed to evaluate: {e}", rule.cel))?;
        if result.as_bool() == Some(true) {
            cells.push(rule.cell);
        } else if !matches!(result, PolicyValue::Bool(_)) {
            return Err(format!("equivalence: fixture rule `{}` did not evaluate to a bool: {result:?}", rule.cel));
        }
    }
    Ok(cells)
}

fn equivalence_task_record(status: &str, at: DateTime<Utc>) -> Value {
    json!({
        "schema": 1,
        "kind": "task",
        "at": at.to_rfc3339(),
        "actor": {"agent_id": "codex-cli"},
        "task_id": "chg-fixture/task-1",
        "title": "fixture task",
        "status": status,
    })
}

fn check_equivalence() -> Result<(), String> {
    let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
    let now = Utc::now();
    // The fixture corpus: every (status, age) combination that crosses
    // one of the two rules' thresholds in either direction.
    let corpus: Vec<(&str, i64)> = vec![("done", 0), ("done", 1), ("done", 7), ("done", 8), ("done", 400), ("open", 0), ("open", 29), ("open", 30), ("open", 31), ("open", 400)];

    for (status, age) in corpus {
        let at = now - Duration::days(age);
        let record = equivalence_task_record(status, at);

        let mut from_cel = cel_required_cells(&bindings, &record, now)?;
        let mut from_static = static_map_required_cells(status, age);
        from_cel.sort_unstable();
        from_static.sort_unstable();

        if from_cel != from_static {
            return Err(format!("equivalence: status={status} age_days={age}: CEL required cells {from_cel:?} != static-map required cells {from_static:?}"));
        }
    }
    Ok(())
}

// ---- rejection (mirrors the former `tests/rejection.rs` body, task
// 6.2: one expression per rejection class, each asserted rejected at
// write time with its "expected …" diagnostic) ----

struct RejectionCase {
    name: &'static str,
    cel: &'static str,
    check: fn(&Diagnostic) -> bool,
}

const REJECTION_CASES: &[RejectionCase] = &[
    RejectionCase {
        name: "undeclared field",
        cel: "record.severty == 'high'",
        check: |d| matches!(d, Diagnostic::UndeclaredField { field, .. } if field == "severty"),
    },
    RejectionCase {
        name: "undeclared bare variable",
        cel: "unknown_var == 1",
        check: |d| matches!(d, Diagnostic::UndeclaredVariable { name, .. } if name == "unknown_var"),
    },
    RejectionCase {
        name: "wrong function arity (too few)",
        cel: "age_days()",
        check: |d| matches!(d, Diagnostic::ArityMismatch { function, expected: 1, got: 0 } if function == "age_days"),
    },
    RejectionCase {
        name: "wrong function arity (too many)",
        cel: "age_days(record.at, record.at)",
        check: |d| matches!(d, Diagnostic::ArityMismatch { function, expected: 1, got: 2 } if function == "age_days"),
    },
    RejectionCase {
        name: "wrong argument type",
        cel: "age_days(record.title)",
        check: |d| matches!(d, Diagnostic::TypeMismatch { function, .. } if function == "age_days"),
    },
    RejectionCase {
        name: "unknown function",
        cel: "mystery_fn(record.title)",
        check: |d| matches!(d, Diagnostic::UnknownFunction { name, .. } if name == "mystery_fn"),
    },
    RejectionCase {
        name: "syntax error",
        cel: "record.title ==",
        check: |d| matches!(d, Diagnostic::Syntax(_)),
    },
    RejectionCase {
        name: "select on a known non-object type (review finding #1)",
        cel: "record.title.foo == 'x'",
        check: |d| matches!(d, Diagnostic::SelectOnNonObject { field, .. } if field == "foo"),
    },
    RejectionCase {
        name: "ordering comparison between incompatible operand types (review finding #2)",
        cel: "record.status > 5",
        check: |d| matches!(d, Diagnostic::OperatorTypeMismatch { operator, .. } if *operator == ">"),
    },
    RejectionCase {
        name: "arithmetic between incompatible operand types (review finding #2)",
        cel: "record.schema + 'x' == 'x'",
        check: |d| matches!(d, Diagnostic::OperatorTypeMismatch { operator, .. } if *operator == "+"),
    },
    RejectionCase {
        name: "pathologically nested comprehensions exceed the compile-time complexity bound (review finding #3)",
        cel: "[1,2,3].map(a, [1,2,3].map(b, [1,2,3].map(c, [1,2,3].map(d, a + b + c + d)))) != []",
        check: |d| matches!(d, Diagnostic::TooComplex { metric, .. } if *metric == "comprehension nesting depth"),
    },
    RejectionCase {
        name: "boolean operator on a known non-bool operand (re-review follow-up on finding #2)",
        cel: "record.title && true",
        check: |d| matches!(d, Diagnostic::OperandTypeMismatch { operator, expected, .. } if *operator == "&&" && *expected == "bool"),
    },
];

fn check_rejection() -> Result<(), String> {
    let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
    for case in REJECTION_CASES {
        match compile(case.cel, &bindings) {
            Ok(_) => return Err(format!("rejection: case `{}` (`{}`): expected rejection, but compile() accepted it", case.name, case.cel)),
            Err(diagnostics) => {
                if !diagnostics.iter().any(case.check) {
                    return Err(format!("rejection: case `{}` (`{}`): no diagnostic matched the expected shape; got {diagnostics:?}", case.name, case.cel));
                }
            }
        }
    }
    Ok(())
}

// ---- determinism (mirrors the former `tests/determinism.rs` body,
// task 6.3: evaluate a set of CEL expressions against fixed input facts
// repeatedly and assert byte-identical results) ----

fn determinism_fixed_now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z").unwrap().with_timezone(&Utc)
}

fn determinism_fixture_records() -> Vec<Value> {
    vec![
        json!({"schema": 1, "kind": "task", "at": "2026-05-01T00:00:00Z", "actor": {"agent_id": "codex-cli"}, "task_id": "chg-a/task-1", "title": "alpha", "status": "done"}),
        json!({"schema": 1, "kind": "task", "at": "2026-01-01T00:00:00Z", "actor": {"agent_id": "claude-code", "role": "implementer"}, "task_id": "chg-b/task-2", "title": "beta", "status": "open"}),
    ]
}

const DETERMINISM_EXPRESSIONS: &[&str] =
    &["record.status == 'done'", "age_days(record.at) > 30", "record.status == 'done' && age_days(record.at) > 7", "has(record.title)", "record.actor.agent_id == 'codex-cli'"];

fn check_determinism() -> Result<(), String> {
    let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
    let now = determinism_fixed_now();

    for expr in DETERMINISM_EXPRESSIONS {
        let compiled = compile(expr, &bindings).map_err(|e| format!("determinism: fixture expression `{expr}` failed write-time validation: {e:?}"))?;
        for record in determinism_fixture_records() {
            let first = evaluate(&compiled, &record, now, EvalBudget::default());
            let second = evaluate(&compiled, &record, now, EvalBudget::default());

            match (&first, &second) {
                (Ok(a), Ok(b)) if a == b => {}
                (Err(a), Err(b)) if format!("{a}") == format!("{b}") => {}
                (Ok(a), Ok(b)) => return Err(format!("determinism: expression `{expr}` against {record}: two evaluations diverged ({a:?} vs {b:?})")),
                (Err(a), Err(b)) => return Err(format!("determinism: expression `{expr}` against {record}: two evaluation errors diverged ({a} vs {b})")),
                _ => return Err(format!("determinism: expression `{expr}` against {record}: one evaluation succeeded and the other failed ({first:?} vs {second:?})")),
            }
        }
    }

    // The many-repeats variant: not just two evaluations agree, but 25.
    let compiled = compile("record.status == 'done' && age_days(record.at) > 7", &bindings)
        .map_err(|e| format!("determinism: many-repeats fixture expression failed write-time validation: {e:?}"))?;
    let record = &determinism_fixture_records()[0];
    let baseline = evaluate(&compiled, record, now, EvalBudget::default()).map_err(|e| format!("determinism: many-repeats baseline evaluation failed: {e}"))?;
    for i in 0..25 {
        let result = evaluate(&compiled, record, now, EvalBudget::default()).map_err(|e| format!("determinism: many-repeats evaluation #{i} failed: {e}"))?;
        if result != baseline {
            return Err(format!("determinism: many-repeats evaluation #{i} diverged from the baseline ({result:?} vs {baseline:?})"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_passes_against_the_real_fixture_flows() {
        match selftest() {
            Ok(passed) => assert_eq!(passed, 3, "expected all 3 CEL fixture flows (equivalence, rejection, determinism) to pass"),
            Err(failures) => panic!("selftest failures: {failures:?}"),
        }
    }
}
