//! The invocations of the rows that are the entry of an agent into the tool (approaches/checker.md 6, 7.1, L2): `skills show`, the
//! stream commands and `schema --search`. A guide's command lines and the first line of `--help` are judged from what these
//! and the L1 run already read.

use serde_json::Value;

use super::super::l1_plan::L1Run;
use super::super::{first_document, in_json_mode, Invocations, Run};
use crate::exec::{Call, RunError};
use crate::fixture::SearchQuery;
use crate::l1::model::CommandDoc;

/// What the rows of this part read.
#[derive(Debug, Default)]
pub(crate) struct GuidesRun {
    pub skills: SkillsRun,
    pub streams: Vec<StreamRun>,
    /// The stream commands the fixture has no `ok` invocation of.
    pub missing_stream: Vec<String>,
    /// Whether the tool's `schema` document has the option `--search`.
    pub search_offered: bool,
    pub searches: Vec<(SearchQuery, usize)>,
}

/// `skills show`, then each guide it lists, then a name that is no guide.
#[derive(Debug, Default)]
pub(crate) struct SkillsRun {
    pub default: Option<usize>,
    pub guides: Vec<(String, usize)>,
    pub unknown: Option<usize>,
    /// The fixture's `ok` invocations of `skills show`, measured: the words and the call.
    pub listed: Vec<(String, usize)>,
}

/// One stream command: the fixture's `ok` invocations of it (already in the log) and a usage error before the start.
#[derive(Debug)]
pub(crate) struct StreamRun {
    pub command: String,
    pub runs: Vec<usize>,
    pub refusal: usize,
    /// The stream outputs of its document: name, event schema name and the event schema, `when`.
    pub outputs: Vec<StreamOutput>,
}

#[derive(Debug, Clone)]
pub(crate) struct StreamOutput {
    pub name: String,
    pub event_ref: Option<String>,
}

impl Run<'_> {
    pub(crate) fn plan_guides(&mut self, calls: &Invocations, l1: &L1Run) -> Result<GuidesRun, RunError> {
        let mut out = GuidesRun::default();
        let default = self.invoke(Call::new(["skills", "show", "--json"]).in_empty_state())?;
        out.skills.default = Some(default);
        let names: Vec<String> = first_document(&self.log[default].stdout)
            .filter(|document| document.get("ok") == Some(&Value::Bool(true)))
            .and_then(|document| document.pointer("/data/guides").cloned())
            .and_then(|guides| guides.as_array().cloned())
            .into_iter()
            .flatten()
            .filter_map(|name| name.as_str().map(str::to_string))
            .collect();
        for name in names.iter().take(16) {
            let call = self.invoke(Call::new(["skills", "show", name.as_str(), "--json"]).in_empty_state())?;
            out.skills.guides.push((name.clone(), call));
        }
        out.skills.unknown = Some(self.invoke(Call::new(["skills", "show", "nosuch-xyz", "--json"]).in_empty_state())?);
        // The fixture's own `skills show` invocations, which run with the fixture's environment: what they write is this row's, never `L2.effects`'.
        let listed: Vec<Vec<String>> = self.fixture.commands.as_ref().and_then(|commands| commands.get("skills show")).map(|entry| entry.ok.clone()).unwrap_or_default();
        for argv in listed {
            let call = self.invoke(Call::new(in_json_mode(&argv)).measured())?;
            out.skills.listed.push((argv.join(" "), call));
        }

        let docs: Vec<CommandDoc> = l1.docs.commands.iter().map(|(id, doc)| CommandDoc { command: id.clone(), ..doc.clone() }).collect();
        for doc in &docs {
            let outputs: Vec<StreamOutput> = doc
                .outputs
                .iter()
                .filter(|output| output.kind == "stream")
                .map(|output| StreamOutput { name: output.name().to_string(), event_ref: output.event_ref.clone() })
                .collect();
            if outputs.is_empty() || self.not_run(&doc.command) {
                continue;
            }
            let mut runs: Vec<usize> = self.ok_bases(calls, doc).into_iter().map(|(_, index)| index).collect();
            let has_ok = !runs.is_empty();
            // A refused invocation that began the stream before it failed is a stream that ended in a failure: the end line is judged too.
            let refused: Vec<usize> = self
                .fixture
                .commands
                .as_ref()
                .into_iter()
                .flatten()
                .zip(&calls.commands)
                .find(|((id, _), _)| id.as_str() == doc.command)
                .map(|(_, command)| command.refused.clone())
                .unwrap_or_default();
            runs.extend(refused.into_iter().filter(|index| {
                first_document(&self.log[*index].stdout).is_some_and(|line| line.get("type").and_then(Value::as_str) == Some("start"))
            }));
            if !has_ok {
                out.missing_stream.push(doc.command.clone());
                continue;
            }
            let mut refusal_argv = doc.path.clone();
            refusal_argv.extend(["--nosuch-xyz-option".to_string(), "--json".to_string()]);
            let refusal = self.invoke(Call::new(refusal_argv))?;
            out.streams.push(StreamRun { command: doc.command.clone(), runs, refusal, outputs });
        }
        out.search_offered = l1.docs.command("schema").is_some_and(|schema| schema.option("search").is_some());
        if out.search_offered {
            for query in self.fixture.search.clone() {
                let call = self.invoke(Call::new(["schema".to_string(), "--search".to_string(), query.query.clone(), "--json".to_string()]))?;
                out.searches.push((query, call));
            }
        }
        Ok(out)
    }
}
