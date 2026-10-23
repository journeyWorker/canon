//! `canon scenario` — the handlers of the group.

use chrono::{Timelike, Utc};
use crate::cli::scenario::*;
use std::process::ExitCode;

pub fn run(cmd: ScenarioCommand) -> ExitCode {
    match cmd {
        ScenarioCommand::New(ScenarioNewArgs { tag, title, subject_id, lane, case, flow, change_id, actor_id, session, feature, project_id, repo, json, input: _ }) => {
            let at = Utc::now().with_nanosecond(0).expect("0 is a valid nanosecond");
            let actor = crate::role::attributed(&actor_id, None, session.session_id.as_ref());
            let spec = crate::scaffold::ScenarioSpec { tag, title, subject: subject_id, lane, case, flow, change: change_id, feature, project: project_id };
            ExitCode::from(crate::scaffold::run_scenario_new(&repo, &spec, &actor, at, json) as u8)
        }
    }
}
