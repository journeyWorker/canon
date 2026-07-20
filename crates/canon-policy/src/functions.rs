//! `canon-policy`'s registered CEL functions (design D2/D4, task 2.2).
//!
//! # Purity audit (task 4.4)
//!
//! `age_days` is the only Rust function this crate registers into a
//! `cel::Context` (`has` is a CEL-native macro, [`crate::bindings`]'s
//! module doc). Audit:
//!
//! - **No filesystem/network access.** The function body performs one
//!   `chrono` duration computation and nothing else.
//! - **No internal wall-clock read.** `now` is a `DateTime<Utc>`
//!   *parameter* to [`register`], supplied by [`crate::eval::evaluate`]'s
//!   own caller (design D4: "evaluation's own 'now' is passed in by the
//!   caller, never read from the wall clock inside the function") — the
//!   closure captures a fixed value, it never calls `Utc::now()` itself.
//!   Two evaluations of the same expression against the same record with
//!   the same caller-supplied `now` are therefore byte-identical (the
//!   determinism fixture, task 6.3, is the mechanical check for this).
//! - **Total, not partial.** A non-timestamp argument produces
//!   `ExecutionError::FunctionError`, a value, never a panic — see the
//!   malformed-input fixture case in `tests/rejection.rs`.
//!
//! Any future addition to this allowlist (design D4's "single-file review
//! point") must preserve all three properties; this module doc is the
//! place a reviewer checks first.

use chrono::{DateTime, Utc};

/// Registers `canon-policy`'s fixed function allowlist into `ctx`, with
/// `now` threaded through as each evaluation call's caller-supplied
/// "current time" (never read from the wall clock internally).
pub(crate) fn register(ctx: &mut cel::Context<'_>, now: DateTime<Utc>) {
    ctx.add_function("age_days", move |ts: cel::Value| -> Result<cel::Value, cel::ExecutionError> { age_days(ts, now) });
}

fn age_days(ts: cel::Value, now: DateTime<Utc>) -> Result<cel::Value, cel::ExecutionError> {
    let cel::Value::Timestamp(ts) = ts else {
        return Err(cel::ExecutionError::function_error("age_days", format!("expected a timestamp argument, got {:?}", ts.type_of())));
    };
    let now = now.fixed_offset();
    let days = now.signed_duration_since(ts).num_days();
    Ok(cel::Value::Int(days))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn age_days_computes_whole_days_since_ts() {
        let now = Utc::now();
        let ts = now - Duration::days(5);
        let result = age_days(cel::Value::Timestamp(ts.fixed_offset()), now).unwrap();
        assert_eq!(result, cel::Value::Int(5));
    }

    #[test]
    fn age_days_on_non_timestamp_is_an_error_value_not_a_panic() {
        let now = Utc::now();
        let result = age_days(cel::Value::Int(1), now);
        assert!(matches!(result, Err(cel::ExecutionError::FunctionError { .. })));
    }
}
