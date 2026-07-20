//! Evaluation: purity, totality, eval budget (design D5, tasks.md group
//! 4). [`evaluate`] only ever accepts a [`CompiledPolicy`] — a raw source
//! string cannot reach this function, so every evaluated expression has
//! already passed [`crate::validate::compile`]'s write-time check.
//!
//! # Purity / totality
//!
//! Evaluation runs against `cel::Context::empty()` (design's closed
//! profile, [`crate::validate`]'s module doc) plus exactly the `record`
//! variable and the `age_days` function ([`crate::functions`]) — no I/O,
//! no ambient state. `Program::execute`'s own `ExecutionError` is a
//! value, and `Program::compile`/`execute` both run inside
//! `catch_unwind` (task 4.2): a malformed call or a `cel` parser/
//! interpreter panic surfaces as `Err(PolicyError)`, never unwinds past
//! this crate's boundary.
//!
//! # Eval budget (design D5, review finding #3 fix — ReviewS13 d26bc9d7)
//!
//! `cel`'s public API exposes no step-counting hook or cancellable
//! execution context (design Risks section — verified against the
//! upstream source, not just the design's original docs.rs 0.10.0 note;
//! re-verified directly against `cel` 0.14.0's own source for this fix:
//! `Program::execute` calls `Value::resolve` directly with no interrupt
//! parameter anywhere in the call chain, and the interpreter's own
//! comprehension loop — `cel-0.14.0/src/objects.rs`, the
//! `Expr::Comprehension` arm of `Value::resolve_val` — is a plain `while
//! let Some(item) = items.next()` with no hook a caller outside the
//! interpreter could use to stop it early). A truly pathological
//! expression therefore CANNOT be aborted mid-evaluation — `evaluate`
//! can choose not to wait for `cel`, but it cannot make `cel` stop.
//!
//! `canon-policy` closes this the way design D5's own Risk note
//! anticipates when no interruptible budget exists: bound the WORST
//! CASE instead of trying to interrupt the AVERAGE case, on both axes
//! evaluation cost depends on:
//! - [`crate::validate::compile`]'s compile-time complexity check (node
//!   count, nesting depth, comprehension count, and comprehension
//!   NESTING depth — the multiplicative-cost dimension a comprehension
//!   CHAIN doesn't have) rejects a structurally pathological expression
//!   before it can ever be stored, let alone reach this function.
//! - [`MAX_RECORD_JSON_NODES`] bounds the one thing the AST check can't
//!   see: a compile-time-legal comprehension (`record.tags.exists(...)`)
//!   still costs work proportional to the RUNTIME size of the list/map
//!   it iterates — the schema only says a field IS a list, never how
//!   long. `evaluate` counts `record`'s total JSON node count and
//!   returns [`PolicyError::RecordTooLarge`] BEFORE spawning the eval
//!   thread at all if it's over the bound — no thread, no work, nothing
//!   to detach.
//!
//! With both bounded, `evaluate` still runs `Program::execute` on a
//! dedicated thread raced against a wall-clock deadline via
//! `mpsc::Receiver::recv_timeout` (unchanged mechanism, kept as a
//! defense-in-depth backstop — NOT the only bound anymore): if the
//! deadline elapses first, `evaluate` returns
//! [`PolicyError::BudgetExceeded`] and the spawned thread is detached
//! (never joined) rather than the calling gate/store operation blocking
//! on it indefinitely. A detach can no longer accumulate genuinely
//! unbounded background work — every expression that thread could be
//! running already passed BOTH the compile-time complexity check and the
//! record-size cap, so its worst case is provably finite; a detach now
//! means "this run happened to be slower than the configured budget on
//! this machine" (design D5's own accepted timing-variance trade-off),
//! never "this will run forever."

use std::collections::{BTreeMap, HashMap};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::registry::CelType;
use crate::validate::CompiledPolicy;

/// A bound on one `evaluate` call's wall-clock execution time (design
/// D5). The default (200ms) is generous relative to the closed,
/// non-recursive expression shapes canon's own consumers need (design
/// Risks section) — a defense-in-depth bound, not a tight resource
/// contract.
#[derive(Debug, Clone, Copy)]
pub struct EvalBudget(pub Duration);

impl Default for EvalBudget {
    fn default() -> Self {
        Self(Duration::from_millis(200))
    }
}

/// A CEL evaluation result, narrowed from `cel::Value` to the shapes a
/// canon-policy predicate/value expression realistically produces.
/// Compound values (`List`/`Map`/…) fall back to a `Debug` rendering —
/// canon's five named consumer touchpoints (`lib.rs` module doc) all read
/// a scalar (routing predicates, aging thresholds, guards), never a
/// structured CEL value.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Double(f64),
    String(String),
    Null,
    Other(String),
}

impl PolicyValue {
    /// The common case: a routing predicate/guard is a CEL expression
    /// that evaluates to a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PolicyValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("CEL evaluation error: {0}")]
    Execution(String),
    #[error("evaluation exceeded its {0:?} budget")]
    BudgetExceeded(Duration),
    #[error("CEL evaluator panicked")]
    Panicked,
    /// `record`'s total JSON node count exceeded [`MAX_RECORD_JSON_NODES`]
    /// — rejected before the eval thread was ever spawned (review
    /// finding #3, ReviewS13 d26bc9d7; see this module's doc for why).
    #[error("record JSON exceeds canon-policy's {limit}-node evaluation bound ({nodes} nodes) — evaluation was never started")]
    RecordTooLarge { nodes: usize, limit: usize },
    /// The compiled expression's comprehension NESTING depth combined
    /// with `record`'s longest list, at eval time, estimates more loop-
    /// body evaluations than [`MAX_ESTIMATED_COMPREHENSION_WORK`] allows
    /// — rejected before the eval thread was ever spawned (re-review
    /// follow-up on ReviewS13 d26bc9d7, found in the 90238dc0
    /// re-review: [`MAX_RECORD_JSON_NODES`] and `crate::validate`'s
    /// compile-time nesting cap were each individually generous, but
    /// their PRODUCT — `longest_list ^ nesting` — was not itself bounded).
    #[error(
        "estimated comprehension work ({longest_list}^{nesting} = {estimated}) exceeds canon-policy's {limit}-iteration evaluation bound — evaluation was never started"
    )]
    EvalWorkTooLarge { estimated: u64, limit: u64, longest_list: usize, nesting: usize },
}

/// A defensive bound on the total JSON node count (every array element,
/// every object value — the same shape [`count_json_nodes`] counts) in
/// the `record` [`evaluate`] converts and hands to `cel` (design D5
/// mitigation, review finding #3, ReviewS13 d26bc9d7; see this module's
/// doc). A compile-time-legal comprehension over `record.tags` still
/// costs work proportional to `tags`'s RUNTIME length — the schema
/// bounds its TYPE, never its length — so this bound covers what
/// [`crate::validate::compile`]'s AST-only complexity check structurally
/// cannot see. 10,000 is generous relative to every real canon record
/// (a Task/Change/Handoff envelope is a few dozen fields deep at most)
/// while still bounding a pathologically large `record` before any eval
/// thread is spawned for it.
const MAX_RECORD_JSON_NODES: usize = 10_000;

fn count_json_nodes(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(items) => 1 + items.iter().map(count_json_nodes).sum::<usize>(),
        serde_json::Value::Object(map) => 1 + map.values().map(count_json_nodes).sum::<usize>(),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) | serde_json::Value::String(_) => 1,
    }
}

/// A defensive bound on the ESTIMATED number of comprehension loop-body
/// evaluations `record` could trigger for `compiled`: `longest_list ^
/// compiled.comprehension_nesting`, where `longest_list` is the length
/// of the LONGEST JSON array reachable anywhere inside `record` (re-
/// review follow-up on ReviewS13 d26bc9d7, found in the 90238dc0
/// re-review). [`MAX_RECORD_JSON_NODES`] and `crate::validate`'s
/// compile-time `MAX_COMPREHENSION_NESTING` were each chosen to be
/// individually generous — but chosen INDEPENDENTLY, their product was
/// not itself bounded: a record at the 10,000-node ceiling combined
/// with an expression at the compile-time nesting ceiling (3) could
/// still estimate up to 10,000^3 (10^12) loop-body evaluations before
/// the wall-clock budget ever fired. 100,000 is still generous for the
/// realistic case (nesting depth 1, i.e. no nesting at all, is by far
/// this crate's most common shape and is effectively unbounded by this
/// check — `longest_list^1` only exceeds 100,000 if `longest_list`
/// already exceeds [`MAX_RECORD_JSON_NODES`], impossible) while closing
/// the interaction gap for depth ≥ 2.
const MAX_ESTIMATED_COMPREHENSION_WORK: u64 = 100_000;

/// The length of the LONGEST JSON array reachable anywhere inside
/// `value` (recursing through both arrays and objects) — see
/// [`MAX_ESTIMATED_COMPREHENSION_WORK`]'s doc for why this, not total
/// node count, is the right per-comprehension-level cost proxy.
fn max_list_len(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(items) => {
            let deepest = items.iter().map(max_list_len).max().unwrap_or(0);
            items.len().max(deepest)
        }
        serde_json::Value::Object(map) => map.values().map(max_list_len).max().unwrap_or(0),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) | serde_json::Value::String(_) => 0,
    }
}

/// Evaluates `compiled` against `record` (the record's own JSON
/// representation — `serde_json::to_value(&some_canon_model_record)`) and
/// `now` (the caller-supplied "current time" `age_days` uses, design D4 —
/// never read from the wall clock internally), bounded by `budget`.
pub fn evaluate(compiled: &CompiledPolicy, record: &serde_json::Value, now: DateTime<Utc>, budget: EvalBudget) -> Result<PolicyValue, PolicyError> {
    let nodes = count_json_nodes(record);
    if nodes > MAX_RECORD_JSON_NODES {
        return Err(PolicyError::RecordTooLarge { nodes, limit: MAX_RECORD_JSON_NODES });
    }
    if compiled.comprehension_nesting > 0 {
        let longest_list = max_list_len(record);
        let estimated = (longest_list as u64).checked_pow(compiled.comprehension_nesting as u32).unwrap_or(u64::MAX);
        if estimated > MAX_ESTIMATED_COMPREHENSION_WORK {
            return Err(PolicyError::EvalWorkTooLarge {
                estimated,
                limit: MAX_ESTIMATED_COMPREHENSION_WORK,
                longest_list,
                nesting: compiled.comprehension_nesting,
            });
        }
    }

    let source = compiled.source.clone();
    let record_fields = compiled.record_fields.clone();
    let record_json = record.clone();

    let (tx, rx) = mpsc::channel();
    let spawned = thread::Builder::new().name("canon-policy-eval".to_string()).spawn(move || {
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| run(&source, &record_fields, &record_json, now)));
        // The receiver may already be gone (budget exceeded, caller moved
        // on) — a dropped-receiver send error is expected, not a bug.
        let _ = tx.send(outcome);
    });
    let handle = match spawned {
        Ok(handle) => handle,
        Err(_) => return Err(PolicyError::Panicked),
    };

    match rx.recv_timeout(budget.0) {
        Ok(Ok(Ok(value))) => {
            drop(handle.join());
            Ok(to_policy_value(value))
        }
        Ok(Ok(Err(exec_err))) => {
            drop(handle.join());
            Err(PolicyError::Execution(exec_err.to_string()))
        }
        Ok(Err(_)) => Err(PolicyError::Panicked),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // Detach: never block the caller waiting for a pathological
            // expression to finish (design D5).
            drop(handle);
            Err(PolicyError::BudgetExceeded(budget.0))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(PolicyError::Panicked),
    }
}

fn run(source: &str, record_fields: &BTreeMap<String, CelType>, record_json: &serde_json::Value, now: DateTime<Utc>) -> cel::ResolveResult {
    // Re-parsing a pre-validated source is deterministic and cheap; it
    // avoids needing `cel::Program` to be `Send` across the eval thread
    // boundary.
    let program = cel::Program::compile(source).expect("source was validated by crate::validate::compile before a CompiledPolicy could exist");
    let mut ctx = cel::Context::empty();
    let record_value = json_to_cel(record_json, &CelType::Map(record_fields.clone()));
    ctx.add_variable_from_value("record", record_value);
    crate::functions::register(&mut ctx, now);
    program.execute(&ctx)
}

fn to_policy_value(value: cel::Value) -> PolicyValue {
    match value {
        cel::Value::Bool(b) => PolicyValue::Bool(b),
        cel::Value::Int(i) => PolicyValue::Int(i),
        cel::Value::UInt(u) => PolicyValue::UInt(u),
        cel::Value::Float(f) => PolicyValue::Double(f),
        cel::Value::String(s) => PolicyValue::String(s.to_string()),
        cel::Value::Null => PolicyValue::Null,
        other => PolicyValue::Other(format!("{other:?}")),
    }
}

/// Converts a JSON value into a CEL value, guided by the schema-derived
/// [`CelType`] tree (the SAME tree [`crate::validate`]'s write-time
/// checker validated field accesses against — one source of type truth
/// for both passes, design D2's invariant applied to runtime conversion
/// as well as static checking).
pub(crate) fn json_to_cel(json: &serde_json::Value, ty: &CelType) -> cel::Value {
    if json.is_null() {
        return cel::Value::Null;
    }
    match ty {
        CelType::Timestamp => match json.as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()) {
            Some(dt) => cel::Value::Timestamp(dt),
            None => cel::Value::Null,
        },
        CelType::String | CelType::Enum(_) => match json.as_str() {
            Some(s) => cel::Value::String(Arc::new(s.to_string())),
            None => cel::Value::Null,
        },
        CelType::Int => json.as_i64().map(cel::Value::Int).unwrap_or(cel::Value::Null),
        CelType::UInt => json.as_u64().map(cel::Value::UInt).unwrap_or(cel::Value::Null),
        CelType::Double => json.as_f64().map(cel::Value::Float).unwrap_or(cel::Value::Null),
        CelType::Bool => json.as_bool().map(cel::Value::Bool).unwrap_or(cel::Value::Null),
        CelType::List(inner) => match json.as_array() {
            Some(items) => cel::Value::List(Arc::new(items.iter().map(|item| json_to_cel(item, inner)).collect())),
            None => cel::Value::Null,
        },
        CelType::Map(fields) => match json.as_object() {
            Some(obj) => {
                let mut map: HashMap<String, cel::Value> = HashMap::with_capacity(obj.len());
                for (key, value) in obj {
                    let field_ty = fields.get(key).unwrap_or(&CelType::Dyn);
                    map.insert(key.clone(), json_to_cel(value, field_ty));
                }
                map.into()
            }
            None => cel::Value::Null,
        },
        CelType::Dyn => json_dyn_to_cel(json),
    }
}

/// Structural JSON→CEL conversion with no schema guidance, for `Dyn`
/// fields (e.g. `Event.detail`) — never a validation gate, since a `Dyn`
/// field is by definition one this crate declined to type.
fn json_dyn_to_cel(json: &serde_json::Value) -> cel::Value {
    match json {
        serde_json::Value::Null => cel::Value::Null,
        serde_json::Value::Bool(b) => cel::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                cel::Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                cel::Value::UInt(u)
            } else {
                cel::Value::Float(n.as_f64().unwrap_or_default())
            }
        }
        serde_json::Value::String(s) => cel::Value::String(Arc::new(s.clone())),
        serde_json::Value::Array(items) => cel::Value::List(Arc::new(items.iter().map(json_dyn_to_cel).collect())),
        serde_json::Value::Object(obj) => {
            let map: HashMap<String, cel::Value> = obj.iter().map(|(k, v)| (k.clone(), json_dyn_to_cel(v))).collect();
            map.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::bindings_for;
    use crate::registry::SchemaRegistry;
    use crate::validate::compile;
    use canon_model::RecordKind;
    use serde_json::json;

    fn task_record() -> serde_json::Value {
        json!({
            "schema": 1,
            "kind": "task",
            "at": "2026-01-01T00:00:00Z",
            "actor": {"agent_id": "codex-cli"},
            "task_id": "chg-a/task-1",
            "title": "do the thing",
            "status": "completed",
        })
    }

    #[test]
    fn evaluates_a_field_comparison_to_bool() {
        let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
        let compiled = compile("record.status == 'completed'", &bindings).unwrap();
        let now = Utc::now();
        let result = evaluate(&compiled, &task_record(), now, EvalBudget::default()).unwrap();
        assert_eq!(result, PolicyValue::Bool(true));
    }

    #[test]
    fn age_days_uses_caller_supplied_now_not_the_wall_clock() {
        let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
        let compiled = compile("age_days(record.at)", &bindings).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-01-11T00:00:00Z").unwrap().with_timezone(&Utc);
        let result = evaluate(&compiled, &task_record(), now, EvalBudget::default()).unwrap();
        assert_eq!(result, PolicyValue::Int(10));
    }

    #[test]
    fn budget_exceeded_never_blocks_the_caller() {
        let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
        // A comprehension over a large synthetic range is enough real
        // work to reliably exceed a near-zero budget without relying on
        // any interpreter-internal timing assumption.
        let compiled = compile("[1,2,3,4,5,6,7,8,9,10].map(x, x * 2).map(x, x * 2).map(x, x * 2) != []", &bindings).unwrap();
        let now = Utc::now();
        let result = evaluate(&compiled, &task_record(), now, EvalBudget(Duration::from_nanos(1)));
        assert!(matches!(result, Err(PolicyError::BudgetExceeded(_))), "expected BudgetExceeded, got {result:?}");
    }

    #[test]
    fn oversized_record_is_rejected_before_evaluation_ever_starts() {
        // Review finding #3 (ReviewS13 d26bc9d7): a compile-time-legal
        // expression can still be handed pathologically large RUNTIME
        // data — `crate::validate::compile`'s AST-only complexity bound
        // can't see this, it's a property of `record`, not of the
        // expression. A generous, non-near-zero budget proves this is
        // the size cap firing, not the wall-clock timeout.
        let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
        let compiled = compile("record.status == 'completed'", &bindings).unwrap();
        let mut record = task_record();
        record["pathological"] = serde_json::Value::Array((0..(MAX_RECORD_JSON_NODES + 1) as i64).map(serde_json::Value::from).collect());
        let now = Utc::now();
        let result = evaluate(&compiled, &record, now, EvalBudget::default());
        assert!(matches!(result, Err(PolicyError::RecordTooLarge { .. })), "expected RecordTooLarge, got {result:?}");
    }

    #[test]
    fn a_pathological_expression_never_reaches_evaluation() {
        // The other half of finding #3: a structurally pathological
        // EXPRESSION (as opposed to oversized runtime data, tested
        // above) is rejected by `crate::validate::compile` itself — it
        // never becomes a `CompiledPolicy` this module could even call
        // `evaluate` on, let alone spawn a thread for.
        let bindings = bindings_for(RecordKind::Task, &SchemaRegistry::load());
        let pathological = "[1,2,3].map(a, [1,2,3].map(b, [1,2,3].map(c, [1,2,3].map(d, a + b + c + d)))) != []";
        assert!(compile(pathological, &bindings).is_err(), "a pathologically nested comprehension must be rejected at compile(), not reach evaluate()");
    }

    #[test]
    fn nested_comprehension_over_an_individually_legal_list_is_rejected_by_the_work_bound() {
        // Re-review follow-up on ReviewS13 d26bc9d7 (found in the
        // 90238dc0 re-review): `record.tags.map(a, record.tags.map(b,
        // ...))` is a depth-2 nested comprehension -- well under
        // `crate::validate`'s MAX_COMPREHENSION_NESTING (3) -- over a
        // 500-element list -- well under MAX_RECORD_JSON_NODES (10,000).
        // NEITHER flat cap alone rejects this, but 500^2 = 250,000
        // estimated loop-body evaluations exceeds
        // MAX_ESTIMATED_COMPREHENSION_WORK (100,000): the PRODUCT of two
        // individually-generous bounds is what this check exists to
        // catch.
        let bindings = bindings_for(RecordKind::Handoff, &SchemaRegistry::load());
        let compiled = compile("record.tags.map(a, record.tags.map(b, a + b)) != []", &bindings).unwrap();
        let tags: Vec<serde_json::Value> = (0..500).map(|i| serde_json::Value::String(format!("t{i}"))).collect();
        let record = json!({ "tags": tags });
        let now = Utc::now();
        let result = evaluate(&compiled, &record, now, EvalBudget::default());
        assert!(matches!(result, Err(PolicyError::EvalWorkTooLarge { .. })), "expected EvalWorkTooLarge, got {result:?}");
    }

    #[test]
    fn a_non_nested_comprehension_over_a_large_list_is_not_penalized_by_the_work_bound() {
        // nesting depth 1 (a single, non-nested comprehension, or a
        // CHAIN of them) must stay bounded ONLY by MAX_RECORD_JSON_NODES
        // -- `longest_list^1` never exceeds
        // MAX_ESTIMATED_COMPREHENSION_WORK unless `longest_list` already
        // exceeds MAX_RECORD_JSON_NODES, which is impossible by the time
        // this check runs (the size cap above already rejected it).
        let bindings = bindings_for(RecordKind::Handoff, &SchemaRegistry::load());
        let compiled = compile("record.tags.exists(a, a == 't1')", &bindings).unwrap();
        let tags: Vec<serde_json::Value> = (0..5000).map(|i| serde_json::Value::String(format!("t{i}"))).collect();
        let record = json!({ "tags": tags });
        let now = Utc::now();
        let result = evaluate(&compiled, &record, now, EvalBudget::default());
        assert!(result.is_ok(), "expected a non-nested comprehension over a large-but-legal list to evaluate, got {result:?}");
    }
}
