import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import trace_check


FIXTURES = Path(__file__).parent / "fixtures"


class TraceCheckTests(unittest.TestCase):
    def report(self, *names, question_turns=()):
        return trace_check.analyze([str(FIXTURES / name) for name in names], question_turns)

    def test_passes_clean_omp_and_separates_reviewer(self):
        report = self.report("pass_impl.jsonl", "pass_review.jsonl")
        self.assertTrue(report["hard_checks_passed"])
        self.assertEqual(report["implementer_sessions"], ["pass-impl"])
        self.assertEqual(report["reviewer_sessions"], ["pass-review"])
        self.assertEqual(report["canon_call_count"], {"review": 1})
        self.assertEqual(report["sessions"][1]["first_command_validity"]["review"]["valid"], True)

    def test_fails_write_before_brief_stop(self):
        report = self.report("prebrief_fail.jsonl")
        self.assertFalse(report["hard_checks_passed"])
        self.assertIn("write_before_brief_stop", {f["check"] for f in report["failures"]})

    def test_fails_write_in_question_only_turn(self):
        report = self.report("question_fail.jsonl", question_turns=(2,))
        self.assertFalse(report["hard_checks_passed"])
        self.assertIn("write_in_question_turn", {f["check"] for f in report["failures"]})

    def test_fails_reviewer_that_also_wrote_code(self):
        report = self.report("reviewer_recovery_fail.jsonl")
        self.assertFalse(report["hard_checks_passed"])
        self.assertIn("reviewer_is_implementer", {f["check"] for f in report["failures"]})

    def test_help_and_out_of_repo_scratch_do_not_change_roles(self):
        # Dogfood eval 8: the implementer read `canon review add --help`, and the
        # reviewer wrote its own harness under /tmp; neither is the other role.
        report = self.report("help_impl.jsonl", "scratch_review.jsonl")
        self.assertNotIn("reviewer_is_implementer", {f["check"] for f in report["failures"]})
        self.assertEqual(report["implementer_sessions"], ["help-impl"])
        self.assertEqual(report["reviewer_sessions"], ["scratch-review"])

    def test_orchestrated_roles_brief_by_subagent_review_from_eval(self):
        # Dogfood eval 9: the planner subagent wrote the brief, so the root's
        # first stop after it is the brief stop; the reviewer recorded its
        # review from an eval cell (Bun.$`canon review add …`).
        report = self.report("orch_root.jsonl")
        self.assertTrue(report["hard_checks_passed"], report["failures"])
        self.assertEqual(report["implementer_sessions"], ["orch-impl"])
        self.assertEqual(report["reviewer_sessions"], ["orch-reviewer"])

    def test_recovery_depth_counts_intervening_canon_calls(self):
        report = self.report("reviewer_recovery_fail.jsonl")
        session = report["sessions"][0]
        recovery = session["recovery_depth"]
        self.assertEqual(len(recovery), 1)
        self.assertEqual(recovery[0]["calls_between"], 0)
        self.assertEqual(session["first_command_validity"]["review"]["valid"], True)
        self.assertEqual(session["first_command_validity"]["status"]["valid"], False)

    def test_reads_codex_exec_jsonl(self):
        report = self.report("codex_pass.jsonl")
        self.assertTrue(report["hard_checks_passed"])
        self.assertEqual(report["sessions"][0]["provider"], "codex")
        self.assertEqual(report["canon_call_count"], {"status": 1})

    def test_json_output_is_serializable(self):
        report = self.report("pass_impl.jsonl")
        encoded = json.dumps(report)
        self.assertIn('"hard_checks_passed": true', encoded)


if __name__ == "__main__":
    unittest.main()
