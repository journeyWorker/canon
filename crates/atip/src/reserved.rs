//! The reserved error codes (spec 7.2.1): one table for every tool and for the checker.
//!
//! A tool registers a reserved code with the exit and retryability given here; [`registration_problem`] is the check that
//! says so, and the checker's `L1.registry` and a tool's own tests call the same function. `STOPPED_MIDWAY` (since 1.2) is
//! reserved only in a document that declares 1.2: in an earlier document it is a code of the tool's own (spec 8.3.2), so
//! [`reserved_code`] and [`registration_problem`] take the minor of the document judged.

use crate::minor::Minor;

/// One reserved code: its name, exit, retryability and the minor that introduced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedCode {
    pub name: &'static str,
    /// The process exit the code carries: `1` or `2`.
    pub exit: u8,
    pub retryable: bool,
    /// The first minor in which the name is reserved; a document of an earlier minor reads the name as the tool's own.
    pub since: Minor,
}

const fn row(name: &'static str, exit: u8, since: Minor) -> ReservedCode {
    ReservedCode { name, exit, retryable: false, since }
}

/// The twelve reserved codes of 1.0 and `STOPPED_MIDWAY` since 1.2, in the order of spec 7.2.1. None is retryable.
pub const RESERVED_CODES: [ReservedCode; 13] = [
    row("USAGE", 2, Minor::V1_0),
    row("UNKNOWN_COMMAND", 2, Minor::V1_0),
    row("COMMAND_REQUIRED", 2, Minor::V1_0),
    row("INTERNAL", 1, Minor::V1_0),
    row("UNVERIFIABLE", 1, Minor::V1_0),
    row("INPUT_INVALID", 2, Minor::V1_0),
    row("INPUT_CONFLICT", 2, Minor::V1_0),
    row("INPUT_TOO_LARGE", 2, Minor::V1_0),
    row("BATCH_INCOMPLETE", 1, Minor::V1_0),
    row("PLAN_STALE", 2, Minor::V1_0),
    row("OPERATION_REUSED", 2, Minor::V1_0),
    row("CURSOR_INVALID", 2, Minor::V1_0),
    row("STOPPED_MIDWAY", 1, Minor::V1_2),
];

/// The reserved row named `name` in a document that declares `minor`: none when the name is not reserved there.
pub fn reserved_code(name: &str, minor: Minor) -> Option<&'static ReservedCode> {
    RESERVED_CODES.iter().find(|reserved| reserved.name == name && minor.at_least(reserved.since))
}

/// What is wrong with a tool registering `code` with `exit` and `retryable` in a document that declares `minor`: `None`
/// when the code is not reserved there, or is registered as reserved. A value the registry does not state is `None` in
/// the arguments and always a problem, so a row that lacks `exit` or `retryable` is not read as right.
pub fn registration_problem(minor: Minor, code: &str, exit: Option<i64>, retryable: Option<bool>) -> Option<String> {
    let reserved = reserved_code(code, minor)?;
    let exit_wrong = exit != Some(i64::from(reserved.exit));
    let retryable_wrong = retryable != Some(reserved.retryable);
    (exit_wrong || retryable_wrong).then(|| {
        format!(
            "{code} is exit {} retryable {}, reserved is exit {} retryable {}",
            exit.map_or("?".to_string(), |exit| exit.to_string()),
            retryable.map_or("?".to_string(), |flag| flag.to_string()),
            reserved.exit,
            reserved.retryable
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atip_crate12_04_the_table_has_thirteen_codes_and_stopped_midway_is_exit_1_not_retryable_since_1_2() {
        assert_eq!(RESERVED_CODES.len(), 13);
        let stopped = RESERVED_CODES.iter().find(|reserved| reserved.name == "STOPPED_MIDWAY").expect("a row");
        assert_eq!((stopped.exit, stopped.retryable, stopped.since), (1, false, Minor::V1_2));
        assert!(RESERVED_CODES.iter().all(|reserved| !reserved.retryable && (reserved.exit == 1 || reserved.exit == 2)));
        let mut names: Vec<&str> = RESERVED_CODES.iter().map(|reserved| reserved.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 13, "one row per name");
        let exits: Vec<(&str, u8)> = RESERVED_CODES.iter().map(|reserved| (reserved.name, reserved.exit)).collect();
        assert_eq!(
            exits,
            [
                ("USAGE", 2),
                ("UNKNOWN_COMMAND", 2),
                ("COMMAND_REQUIRED", 2),
                ("INTERNAL", 1),
                ("UNVERIFIABLE", 1),
                ("INPUT_INVALID", 2),
                ("INPUT_CONFLICT", 2),
                ("INPUT_TOO_LARGE", 2),
                ("BATCH_INCOMPLETE", 1),
                ("PLAN_STALE", 2),
                ("OPERATION_REUSED", 2),
                ("CURSOR_INVALID", 2),
                ("STOPPED_MIDWAY", 1),
            ]
        );
    }

    #[test]
    fn atip_crate12_04_a_code_is_reserved_only_in_a_document_of_the_minor_that_introduced_it() {
        for minor in [Minor::V1_0, Minor::V1_1] {
            assert!(reserved_code("STOPPED_MIDWAY", minor).is_none(), "{minor}");
            assert!(reserved_code("PLAN_STALE", minor).is_some(), "{minor}");
        }
        assert!(reserved_code("STOPPED_MIDWAY", Minor::V1_2).is_some());
        assert!(reserved_code("NOT_RESERVED", Minor::V1_2).is_none());
    }

    #[test]
    fn atip_crate12_05_a_stopped_midway_registered_with_exit_2_fails_in_a_1_2_document_and_not_in_a_1_1_one() {
        let at_12 = registration_problem(Minor::V1_2, "STOPPED_MIDWAY", Some(2), Some(false)).expect("a problem in 1.2");
        assert!(at_12.contains("STOPPED_MIDWAY") && at_12.contains("exit 2") && at_12.contains("reserved is exit 1"), "{at_12}");
        assert_eq!(registration_problem(Minor::V1_1, "STOPPED_MIDWAY", Some(2), Some(false)), None, "a 1.1 document reads it as the tool's own");
        assert_eq!(registration_problem(Minor::V1_0, "STOPPED_MIDWAY", Some(2), Some(true)), None);
        // The code as the contract reserves it passes, and so does a code that is not reserved.
        assert_eq!(registration_problem(Minor::V1_2, "STOPPED_MIDWAY", Some(1), Some(false)), None);
        assert_eq!(registration_problem(Minor::V1_2, "WRITE_FAILED", Some(2), Some(true)), None);
        // A wrong retryability, and a member the registry does not state, are problems.
        assert!(registration_problem(Minor::V1_2, "STOPPED_MIDWAY", Some(1), Some(true)).is_some());
        assert!(registration_problem(Minor::V1_1, "USAGE", Some(2), None).is_some());
        assert!(registration_problem(Minor::V1_1, "USAGE", None, Some(false)).is_some());
        assert!(registration_problem(Minor::V1_1, "PLAN_STALE", Some(1), Some(false)).is_some());
    }
}
