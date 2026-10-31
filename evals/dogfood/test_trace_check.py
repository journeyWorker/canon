import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import trace_check


FIXTURES = Path(__file__).parent / "fixtures"


class TraceCheckTests(unittest.TestCase):
    def test_read_and_write_calls_come_from_the_spec_effect(self):
        effects = trace_check.EFFECTS
        for words in (["gate", "check"], ["status"], ["brief", "show", "add-login"], ["subject", "show", "demo"], ["schema", "evidence", "add"]):
            self.assertEqual(effects[trace_check.leaf_of(words)], "read", words)
        for words in (["evidence", "add", "--x"], ["subject", "move", "demo"], ["task", "done", "a#1"], ["purge"]):
            self.assertNotEqual(effects[trace_check.leaf_of(words)], "read", words)
        self.assertEqual(trace_check.leaf_of(["gate", "task"]), ())  # a removed spelling is not a leaf
        self.assertEqual(trace_check.leaf_of(["brief"]), ())  # a bare group is not runnable

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
                         [("canon finding list --change-id x", 2, None, "naming"),
                          ("canon change show --change missing --json", 1, "NOT_FOUND", "other"),
                          ("canon finding list --change-id x", 2, "USAGE", "naming")])
        rendered = trace_check.render(report)
        self.assertIn("Wrong-shape canon calls: 2 naming, 1 other", rendered)
        self.assertNotIn("deprecated_spellings", report)

    def test_json_usage_error_naming_and_kind_value(self):
        self.assertEqual(trace_check.classify_failure(
            '{"error":{"code":"USAGE","message":"error: unexpected argument \'--project-id\' found"}} --project'), "naming")
        self.assertEqual(trace_check.classify_failure("error: invalid value 'review_record' for '--kind <KIND>'"), "naming")
        self.assertEqual(trace_check.classify_failure("error: unexpected argument '--zzz' found"), "other")
        self.assertEqual(trace_check.error_code("canon: GATE_FAILED: nope"), "GATE_FAILED")
        self.assertEqual(trace_check.classify_failure(
            '{"error":{"code":"USAGE","hint":"--actor-id was removed in 0.31.0; use `canon finding add --actor x`"}}'), "naming")
        self.assertEqual(trace_check.classify_failure(
            "hint: --kind review_record was renamed --kind review in 0.24.0; use --kind review"), "naming")
        self.assertEqual(trace_check.classify_failure(
            '{"error":{"code":"USAGE","hint":"--agent-id was removed in 0.28.0; use `--actor`"}}'), "naming")


def failure(command, code="USAGE", message="unexpected argument '--x' found", hint=""):
    """An atip/1 JSON failure envelope as `canon <command> --json` prints it."""
    return json.dumps({"ok": False, "error": {"code": code, "message": message, "hint": hint, "retryable": False},
                       "meta": {"schemaVersion": "1.0", "tool": "canon", "toolVersion": "0.33.0", "contract": "atip/1", "command": command}}, separators=(",", ":"))


class SyntheticTranscriptTests(unittest.TestCase):
    """Short hand-written transcripts: one bash call per `(command, output)` pair."""

    def run_calls(self, *pairs, session="syn"):
        rows = [{"type": "session", "version": 3, "id": session, "timestamp": "2026-01-01T00:00:00Z", "cwd": "/workspace"},
                {"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "go"}]}, "timestamp": "2026-01-01T00:00:01Z"}]
        for n, (command, output) in enumerate(pairs, 1):
            rows.append({"type": "message", "message": {"role": "assistant", "content": [
                {"type": "toolCall", "id": f"c{n}", "name": "bash", "arguments": {"command": command, "cwd": "/workspace"}}]},
                "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
            rows.append({"type": "message", "message": {"role": "toolResult", "toolCallId": f"c{n}", "isError": False,
                                                        "content": [{"type": "text", "text": output}]},
                         "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "session.jsonl"
            path.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
            return trace_check.analyze([str(path)])

    def wrong(self, report):
        return [(i["command"].split(" --")[0], i["code"], i["class"]) for i in report["wrong_shape_calls"]["items"]]

    def test_a_usage_envelope_behind_a_chain_is_a_wrong_shape(self):
        # The exit status of `a; b` is b's, so only the envelope shows `a` failed.
        out = failure("subject new") + "\n# README"
        report = self.run_calls(("canon subject new demo --domain dev --title T --session s --json; cat README.md", out))
        self.assertEqual(self.wrong(report), [("canon subject new demo", "USAGE", "other")])

    def test_removed_spelling_in_the_envelope_is_naming(self):
        out = failure("brief check", message="--change-id was removed in 0.28.0", hint="use `canon brief check pause-menu`")
        report = self.run_calls(("canon brief check --change-id pause-menu --json | head -c 700", out))
        self.assertEqual(self.wrong(report), [("canon brief check", "USAGE", "naming")])
        self.assertEqual(report["wrong_shape_calls"]["naming"], 1)

    def test_input_and_unknown_field_codes_are_shape_errors_but_a_refusal_is_not(self):
        report = self.run_calls(
            ("canon scenario new --input '{\"bogus\":1}' --json", failure("scenario new", "INPUT_INVALID", "`bogus` is not an option")),
            ("canon query --kind nope --json", failure("query", "UNKNOWN_COMMAND", "no such command")),
            ("canon evidence add --scenario a.b.01 --json", failure("evidence add", "BRIEF_UNAPPROVED", "brief not approved")))
        self.assertEqual(self.wrong(report), [("canon scenario new", "INPUT_INVALID", "other"), ("canon query", "UNKNOWN_COMMAND", "other")])
        self.assertEqual([r["code"] for r in report["refusals"]], ["BRIEF_UNAPPROVED"])

    def test_two_failures_in_one_call_are_two_wrong_shapes(self):
        out = failure("review scenario", message="unexpected argument '--summary'") + "\n" + failure("review scenario", message="unexpected argument '--summary'")
        report = self.run_calls(("canon review scenario a.b.01 --summary x --json; canon review scenario a.b.02 --summary y --json", out))
        self.assertEqual(len(report["wrong_shape_calls"]["items"]), 2)

    def test_a_truncated_envelope_still_counts(self):
        out = failure("query")[:90]  # `head -c` cut the document short
        report = self.run_calls(("canon query --kind evidence --scenario a.b.01 --json | head -c 90", out))
        self.assertEqual(self.wrong(report), [("canon query", "USAGE", "other")])

    def test_an_envelope_no_visible_call_explains_is_still_counted(self):
        report = self.run_calls(("./run-checks.sh", failure("finding add")))
        self.assertEqual(self.wrong(report), [("canon finding add", "USAGE", "other")])
        self.assertEqual(report["canon_call_count"], {"finding": 1})

    def test_a_clap_usage_error_without_an_exit_code_is_a_wrong_shape(self):
        out = "error: unexpected argument '--session' found\n\nUsage: canon subject new --domain <NAME> <SUBJECT_ID>\n"
        report = self.run_calls(("canon subject new demo --domain dev --title T --session s 2>&1 | head", out))
        self.assertEqual(self.wrong(report), [("canon subject new demo", "USAGE", "other")])

    def test_calls_behind_env_prefixes_pipes_loops_and_shell_strings_are_seen(self):
        report = self.run_calls(
            ("env -u CANON_ROLE canon status --json | jq .data", ""),
            ("env CANON_ROLE=reviewer canon gate check --json", ""),
            ("CANON_ROLE=planner canon brief check x --json && echo ok", ""),
            ("for s in a.b.01 a.b.02; do canon evidence add --scenario $s --class test-run --ref t; done", ""),
            ("bash -c 'canon query --kind evidence --json | wc -l'", ""),
            ("cd repo && timeout 60 canon finding list --change x", ""),
            ("python3 -c \"import subprocess; subprocess.run(['canon', 'review', 'scenario', 'a.b.01'])\"", ""),
            ("python3 - <<'EOF'\nimport subprocess\nsubprocess.run('canon task done x#1', shell=True)\nEOF", ""),
            ("canon schema finding add --json 2>&1 | python3 -c \"\nimport json,sys\nprint(json.load(sys.stdin))\"; canon status", ""))
        self.assertEqual(report["canon_call_count"], {"brief": 1, "evidence": 1, "finding": 1, "gate": 1, "query": 1,
                                                       "review": 1, "schema": 1, "status": 2, "task": 1})

    def test_probes_are_counted_per_session_and_in_total(self):
        report = self.run_calls(
            ("canon finding add --help; canon review scenario --help", ""),
            ("canon schema evidence add --json | head -50", ""),
            ("canon status --json", ""),
            ("canon query -h", ""))
        self.assertEqual({k: report["probes"][k] for k in ("help", "schema", "total")}, {"help": 3, "schema": 1, "total": 4})
        self.assertEqual(report["probes"]["per_session"], {"syn": 4})
        self.assertIn("Probes (--help/-h, canon schema): 4 (3 help, 1 schema)", trace_check.render(report))

    def test_a_human_only_leaf_run_with_the_role_stripped_fails_the_hard_check(self):
        for command in ("env -u CANON_ROLE canon brief approve pause-menu --approver me --note ok --json",
                        "unset CANON_ROLE; canon policy approve --approver me",
                        "CANON_ROLE= canon divergence accept a.b.01 --actor me --reason r --tracking t"):
            report = self.run_calls((command, ""))
            self.assertFalse(report["hard_checks_passed"], command)
            self.assertEqual([f["check"] for f in report["failures"]], ["human_only_role_stripped"], command)

    def test_a_human_only_leaf_without_the_role_after_the_session_declared_one_fails(self):
        report = self.run_calls(("canon status --as orchestrator --json", ""),
                                ("canon brief approve pause-menu --approver me --note ok --json", ""))
        self.assertEqual([f["check"] for f in report["failures"]], ["human_only_role_stripped"])

    def test_a_refused_human_only_call_is_not_a_violation(self):
        refusal = failure("brief approve", "HUMAN_ONLY", "not available to the planner role")
        report = self.run_calls(("canon status --as planner --json", ""), ("canon brief approve x --approver me --note n --json", refusal))
        self.assertTrue(report["hard_checks_passed"])
        self.assertEqual(report["wrong_shape_calls"]["items"], [])
        self.assertEqual([r["code"] for r in report["refusals"]], ["HUMAN_ONLY"])

    def test_the_human_only_help_and_a_role_less_session_are_clean(self):
        report = self.run_calls(("canon brief approve --help", ""), ("canon brief approve x --approver me --note n --json", ""))
        self.assertTrue(report["hard_checks_passed"])

    def run_named(self, name, *pairs):
        """`run_calls` for a subagent transcript file named `<name>.jsonl`."""
        rows = [{"type": "session", "version": 3, "id": name, "timestamp": "2026-01-01T00:00:00Z", "cwd": "/workspace"},
                {"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "go"}]}, "timestamp": "2026-01-01T00:00:01Z"}]
        for n, (command, output) in enumerate(pairs, 1):
            rows.append({"type": "message", "message": {"role": "assistant", "content": [
                {"type": "toolCall", "id": f"c{n}", "name": "bash", "arguments": {"command": command, "cwd": "/workspace"}}]},
                "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
            rows.append({"type": "message", "message": {"role": "toolResult", "toolCallId": f"c{n}", "isError": False,
                                                        "content": [{"type": "text", "text": output}]},
                         "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / f"{name}.jsonl"
            path.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
            return trace_check.analyze([str(path)])

    def test_a_subagent_named_for_the_reviewer_is_a_reviewer_session(self):
        # No review call at all: the transcript name alone says what it was spawned as.
        report = self.run_named("PauseReReviewer", ("canon status --json", ""))
        self.assertEqual(report["reviewer_sessions"], ["PauseReReviewer"])
        self.assertEqual(self.run_named("PauseImplementer", ("canon status --json", ""))["reviewer_sessions"], [])

    def test_a_declared_reviewer_role_makes_a_reviewer_session(self):
        for command in ("canon gate check --as reviewer --json", "CANON_ROLE=reviewer canon gate check --json",
                        "env CANON_ROLE=reviewer canon gate check --json"):
            self.assertEqual(self.run_calls((command, ""))["reviewer_sessions"], ["syn"], command)
        self.assertEqual(self.run_calls(("canon review scenario a.b.01 --actor r --original-spec-ref x --json", ""))["reviewer_sessions"], ["syn"])
        self.assertEqual(self.run_calls(("canon gate check --as implementer --json", ""))["reviewer_sessions"], [])

    def test_canon_owned_projection_writes_are_not_agent_writes_before_the_brief_stop(self):
        # A write before the stop is a violation; `skills install` rewriting the provider files is not one.
        rows = [("canon skills install", ""), ("canon migrate --commit --plan-digest abc --json", ""),
                ("canon change new c1 --subject s --title T --json", ""), ("printf x > notes.txt", "")]
        with tempfile.TemporaryDirectory() as tmp:
            trace = trace_check.analyze([self._brief_stop_session(tmp, rows)])
        paths = [f["path"] for f in trace["failures"] if f["check"] == "write_before_brief_stop"]
        self.assertEqual(paths, ["notes.txt"], "only the agent's own write counts")

    def _brief_stop_session(self, tmp, pairs):
        rows = [{"type": "session", "version": 3, "id": "stop", "timestamp": "2026-01-01T00:00:00Z", "cwd": "/workspace"},
                {"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "go"}]}, "timestamp": "2026-01-01T00:00:01Z"}]
        for n, (command, output) in enumerate(pairs, 1):
            rows.append({"type": "message", "message": {"role": "assistant", "content": [
                {"type": "toolCall", "id": f"c{n}", "name": "bash", "arguments": {"command": command, "cwd": "/workspace"}}]},
                "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
            rows.append({"type": "message", "message": {"role": "toolResult", "toolCallId": f"c{n}", "isError": False,
                                                        "content": [{"type": "text", "text": output}]},
                         "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
        n = len(pairs) + 1
        rows.append({"type": "message", "message": {"role": "assistant", "content": [
            {"type": "toolCall", "id": "w", "name": "write", "arguments": {"path": ".canon/changes/c1/brief.md", "content": "b"}}]},
            "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
        rows.append({"type": "message", "message": {"role": "toolResult", "toolCallId": "w", "isError": False, "content": [{"type": "text", "text": "ok"}]},
                     "timestamp": f"2026-01-01T00:00:{n + 1:02d}Z"})
        rows.append({"type": "message", "message": {"role": "assistant", "content": [], "stopReason": "stop"}, "timestamp": f"2026-01-01T00:00:{n + 2:02d}Z"})
        path = Path(tmp) / "stop.jsonl"
        path.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
        return str(path)

    def test_a_subagent_is_linked_to_its_root_by_file_name_when_the_recorded_path_is_elsewhere(self):
        def rows(sid, parent=None):
            header = {"type": "session", "version": 3, "id": sid, "timestamp": "2026-01-01T00:00:00Z", "cwd": "/workspace"}
            if parent:
                header["parentSession"] = parent
            return [header, {"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": "go"}]}, "timestamp": "2026-01-01T00:00:01Z"}]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "2026-01-01T00-00-00-000Z_root.jsonl"
            root.write_text("\n".join(json.dumps(r) for r in rows("root")) + "\n")
            (Path(tmp) / root.stem).mkdir()
            child = Path(tmp) / root.stem / "PauseReviewer.jsonl"
            child.write_text("\n".join(json.dumps(r) for r in rows("child", "/other/machine/sessions/" + root.name)) + "\n")
            report = trace_check.analyze([str(root)])
        self.assertEqual({s["id"]: s["parent"] for s in report["sessions"]}, {"root": None, "child": "root"})
        self.assertEqual(report["reviewer_sessions"], ["child"])

    def test_a_command_less_envelope_is_attributed_to_the_call_that_produced_it(self):
        # `head -c` cut the document before `meta.command`, and the call took its options from `$A`.
        cut = '{"ok":false,"error":{"code":"USAGE","message":"unexpected argument \'--actor\' found","field":"--actor","expected":["--repo","--release","--json"],"received'
        command = ('A="--as reviewer --json --actor r-1 --session s-1"\n'
                   'canon finding add --change c --round 1 --seq 1 --severity note $A --summary x\n'
                   'canon gate check $A | head -c 160\n'
                   'canon status --json | head -c 50')
        report = self.run_calls((command, '{"ok":true,"data":{}}\n' + cut))
        items = report["wrong_shape_calls"]["items"]
        self.assertEqual([(i["command"].split(" --as")[0], i["code"]) for i in items], [("canon gate check", "USAGE")])

    def test_a_command_named_in_the_message_attributes_a_cut_envelope(self):
        cut = '{"ok":false,"error":{"code":"USAGE","message":"`canon query` has no field `key`","field":"--fields","expected":["actor","at'
        report = self.run_calls(("canon gate check --json | head -c 100\ncanon query --kind evidence --fields key --json | head -c 300", '{"ok":true}\n' + cut))
        self.assertEqual([i["command"].split(" --")[0] for i in report["wrong_shape_calls"]["items"]], ["canon query"])

    def test_an_ambiguous_cut_envelope_lists_the_candidate_commands(self):
        cut = '{"ok":false,"error":{"code":"USAGE","message":"boom","field":"--x","expected":["--a'
        report = self.run_calls(("canon status --json | head -c 10; canon query --kind run --json | head -c 10", cut))
        [item] = report["wrong_shape_calls"]["items"]
        self.assertIn("canon status", item["command"])
        self.assertIn("canon query", item["command"])
        self.assertNotIn("<unseen>", item["command"])


if __name__ == "__main__":
    unittest.main()
