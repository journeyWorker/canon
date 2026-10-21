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

    def test_scratch_outside_the_repo_is_neither_code_nor_a_pre_stop_write(self):
        # Dogfood eval 10: a planner wrote notes under /tmp before its brief
        # stop, and a reviewer wrote `$TMPDIR/det.ts` (an unexpanded variable).
        for path in ("../../../tmp/notes.md", "/tmp/x.ts", "$TMPDIR/det.ts", "${HOME}/y.ts"):
            self.assertTrue(trace_check.outside_repo(path), path)
            self.assertFalse(trace_check.is_code_path(path), path)
        for path in ("src/game.ts", ".canon/changes/x/brief.md", "e2e/a.spec.ts"):
            self.assertFalse(trace_check.outside_repo(path), path)

    def test_only_code_that_landed_in_git_counts_as_implementation(self):
        # Dogfood eval 11: a reviewer wrote a diagnostic spec in the repo and
        # never committed it; only committed code implements the change.
        import subprocess, tempfile
        with tempfile.TemporaryDirectory() as root:
            run = lambda *a: subprocess.run(["git", "-C", root, *a], check=True, capture_output=True)
            run("init", "-q")
            (Path(root) / "src.ts").write_text("x")
            run("add", "src.ts")
            run("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "c")
            (Path(root) / "diag.spec.ts").write_text("y")
            self.assertTrue(trace_check.landed(root, "src.ts"))
            self.assertFalse(trace_check.landed(root, "diag.spec.ts"))
        with tempfile.TemporaryDirectory() as plain:
            self.assertTrue(trace_check.landed(plain, "src.ts"), "outside a git work tree every path counts")

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

    def test_counts_naming_failures_and_removed_spellings(self):
        report = self.report("naming_failures.jsonl")
        self.assertTrue(report["hard_checks_passed"])
        wrong = report["wrong_shape_calls"]
        self.assertEqual((wrong["naming"], wrong["other"]), (2, 1))
        self.assertEqual([(i["command"], i["exit_code"], i["code"], i["class"]) for i in wrong["items"]],
                         [("canon finding list --change x", 2, None, "naming"),
                          ("canon change show --change-id missing --json", 1, "NOT_FOUND", "other"),
                          ("canon finding list --change x", 2, "USAGE", "naming")])
        rendered = trace_check.render(report)
        self.assertIn("Wrong-shape canon calls: 2 naming, 1 other", rendered)
        self.assertNotIn("deprecated_spellings", report)

    def test_json_usage_error_naming_and_kind_value(self):
        self.assertEqual(trace_check.classify_failure(
            '{"error":{"code":"USAGE","message":"error: unexpected argument \'--project\' found"}} --project-id'), "naming")
        self.assertEqual(trace_check.classify_failure("error: invalid value 'review_record' for '--kind <KIND>'"), "naming")
        self.assertEqual(trace_check.classify_failure("error: unexpected argument '--zzz' found"), "other")
        self.assertEqual(trace_check.error_code("canon gate check: GATE_FAILED \u2014 nope"), "GATE_FAILED")
        self.assertEqual(trace_check.classify_failure(
            '{"error":{"code":"USAGE","hint":"--actor was renamed --actor-id in 0.24.0; use --actor-id"}}'), "naming")
        self.assertEqual(trace_check.classify_failure(
            "hint: --kind review_record was renamed --kind review in 0.24.0; use --kind review"), "naming")


if __name__ == "__main__":
    unittest.main()
