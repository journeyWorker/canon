//! `canon scenario` — the handlers of the group.

use canon_model::{Actor};
use chrono::{Timelike, Utc};
use crate::cli::scenario::*;
use std::process::ExitCode;

pub fn run(cmd: ScenarioCommand) -> ExitCode {
    match cmd {
        ScenarioCommand::New(ScenarioNewArgs { tag, from, title, subject_id, lane, case, flow, change_id, actor_id, feature, project_id, repo, json }) => {
            let at = Utc::now().with_nanosecond(0).expect("0 is a valid nanosecond");
            let actor = Actor::new_unattributed(&actor_id);
            let code = match (from, tag, title) {
                (Some(source), _, _) => crate::scaffold::run_scenario_batch(&repo, &source, &actor, at, json),
                (None, Some(tag), Some(title)) => {
                    let spec = crate::scaffold::ScenarioSpec { tag, title, subject: subject_id, lane, case, flow, change: change_id, feature, project: project_id };
                    crate::scaffold::run_scenario_new(&repo, &spec, &actor, at, json)
                }
                (None, _, _) => unreachable!("clap requires <tag> and --title unless --from is given"),
            };
            ExitCode::from(code as u8)
        }
    }
}
