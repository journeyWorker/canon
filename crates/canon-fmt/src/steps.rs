//! The step index of a `.feature` file (0.30 `record-links`).
//!
//! `canon inventory sync` puts each scenario's steps on its index record. They
//! are read with the standard Gherkin parser (the `gherkin` crate, an owner
//! decision), not with the line scan of [`crate::gherkin`]: the scan finds the
//! scenario ids, titles and tags, and [`crate::gherkin::scenario_digests`] still
//! computes the scenario digest, so no digest moves. This module only answers
//! "which steps does the scenario with this id have, in source order".
//!
//! A scenario's steps are its Feature `Background:` steps, then its enclosing
//! `Rule:`'s `Background:` steps, then its own, each inherited step marked with
//! the block it came from ([`StepOrigin`]). A scenario is identified by the id
//! tags on the scenario itself (the same tags the line scan pairs with it);
//! every id tag of one scenario gets the same steps.

use std::collections::BTreeMap;

use canon_model::records::{ScenarioStep, StepOrigin};
use gherkin::{Feature, GherkinEnv, Scenario, Step};

/// A `.feature` file the Gherkin parser refused: the parser's own message
/// (`Error at <line>:<column>: <what it expected>`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StepsError(pub String);

/// The steps of every scenario in `text` that carries a scenario-id tag, by id.
pub fn scenario_steps(text: &str) -> Result<BTreeMap<String, Vec<ScenarioStep>>, StepsError> {
    let feature = Feature::parse(text, GherkinEnv::default()).map_err(|e| StepsError(e.to_string()))?;
    let mut steps = BTreeMap::new();
    let feature_background = feature.background.as_ref().map_or(&[][..], |background| background.steps.as_slice());
    for scenario in &feature.scenarios {
        index(&mut steps, scenario, feature_background, &[]);
    }
    for rule in &feature.rules {
        let rule_background = rule.background.as_ref().map_or(&[][..], |background| background.steps.as_slice());
        for scenario in &rule.scenarios {
            index(&mut steps, scenario, feature_background, rule_background);
        }
    }
    Ok(steps)
}

fn index(steps: &mut BTreeMap<String, Vec<ScenarioStep>>, scenario: &Scenario, feature_background: &[Step], rule_background: &[Step]) {
    let own: Vec<ScenarioStep> = feature_background
        .iter()
        .map(|step| step_of(step, Some(StepOrigin::Background)))
        .chain(rule_background.iter().map(|step| step_of(step, Some(StepOrigin::Rule))))
        .chain(scenario.steps.iter().map(|step| step_of(step, None)))
        .collect();
    for tag in &scenario.tags {
        let id = tag.strip_prefix('@').unwrap_or(tag);
        if crate::gherkin::looks_like_scenario_id(id) {
            steps.insert(id.to_string(), own.clone());
        }
    }
}

fn step_of(step: &Step, inherited: Option<StepOrigin>) -> ScenarioStep {
    ScenarioStep { keyword: step.keyword.trim().to_string(), text: step.value.trim().to_string(), inherited }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEATURE: &str = "Feature: cart\n  # canon: {}\n  Background:\n    Given a signed-in shopper\n\n  @cart.add.01\n  Scenario: Add one\n    Given an empty cart\n    When one item is added\n    Then the cart holds 1\n    And the total is shown\n    But no coupon applies\n\n  Rule: Limits\n    Background:\n      Given a limit of 3\n\n    @cart.add.02 @case:failure\n    Scenario: Add past the limit\n      When 4 items are added\n      Then the add is refused\n";

    #[test]
    fn steps_come_in_source_order_with_inherited_ones_first_and_marked() {
        let steps = scenario_steps(FEATURE).unwrap();
        let plain = |keyword: &str, text: &str, inherited| ScenarioStep { keyword: keyword.into(), text: text.into(), inherited };
        assert_eq!(
            steps["cart.add.01"],
            vec![
                plain("Given", "a signed-in shopper", Some(StepOrigin::Background)),
                plain("Given", "an empty cart", None),
                plain("When", "one item is added", None),
                plain("Then", "the cart holds 1", None),
                plain("And", "the total is shown", None),
                plain("But", "no coupon applies", None),
            ]
        );
        assert_eq!(
            steps["cart.add.02"],
            vec![
                plain("Given", "a signed-in shopper", Some(StepOrigin::Background)),
                plain("Given", "a limit of 3", Some(StepOrigin::Rule)),
                plain("When", "4 items are added", None),
                plain("Then", "the add is refused", None),
            ]
        );
        assert_eq!(steps.len(), 2, "only scenarios with a scenario-id tag are indexed");
    }

    #[test]
    fn a_file_the_parser_refuses_is_an_error_naming_the_place() {
        let error = scenario_steps("Scenario: y\n  Given a step\n").unwrap_err();
        assert!(error.0.starts_with("Error at "), "{error}");
    }
}
