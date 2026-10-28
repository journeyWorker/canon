#!/usr/bin/env python3
"""Offline fixture tests for supply-chain checks in the release checker."""

from __future__ import annotations

import importlib.util
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parent
FIXTURES = ROOT / "test-fixtures" / "release-safety"
SPEC = importlib.util.spec_from_file_location(
    "check_release_workflow_safety", ROOT / "check-release-workflow-safety.py"
)
assert SPEC and SPEC.loader
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class SupplyChainFixtureTests(unittest.TestCase):
    def errors(self, name: str) -> list[str]:
        root = FIXTURES / name
        tracked = [path.relative_to(root) for path in root.rglob("*") if path.is_file()]
        return CHECKER.supply_chain_errors(root, tracked)

    def test_accepts_pinned_actions_digests_checksums_and_lockfiles(self) -> None:
        self.assertEqual(self.errors("accept"), [])

    def test_ignores_binary_assets_and_non_release_fixtures(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            scripts = root / "scripts"
            scripts.mkdir()
            tracked = []
            for suffix in (
                ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico",
                ".parquet", ".wasm", ".zip", ".tar", ".gz", ".bin", ".exe",
                ".PNG", ".pdf", ".woff2", ".sqlite",
            ):
                relative = pathlib.Path("scripts") / f"compose-fixture{suffix}"
                (root / relative).write_bytes(b"\xff\xfe\x00")
                tracked.append(relative)
            fixture = pathlib.Path("fixtures") / "opaque-artifact"
            (root / fixture).parent.mkdir()
            (root / fixture).write_bytes(b"\xff\xfe\x00")
            tracked.append(fixture)
            workflow = pathlib.Path(".github/workflows/release.yml")
            (root / workflow).parent.mkdir(parents=True)
            (root / workflow).write_text(
                "jobs:\n  release:\n    steps:\n      - uses: actions/checkout@v4\n",
                encoding="utf-8",
            )
            tracked.append(workflow)

            errors = CHECKER.supply_chain_errors(root, tracked)

            self.assertEqual(len(errors), 1, errors)
            self.assertIn(".github/workflows/release.yml:", errors[0])
            self.assertIn("full 40-hex commit SHA", errors[0])

    def test_rejects_mutable_action_ref(self) -> None:
        errors = self.errors("reject-action-ref")
        self.assertTrue(any("full 40-hex commit SHA" in error for error in errors))

    def test_rejects_mutable_ref_in_composite_action(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            action = pathlib.Path(".github/actions/setup-rust/action.yml")
            (root / action).parent.mkdir(parents=True)
            (root / action).write_text(
                "runs:\n  using: composite\n  steps:\n    - uses: actions/cache@v5\n",
                encoding="utf-8",
            )

            errors = CHECKER.supply_chain_errors(root, [action])

            self.assertEqual(len(errors), 1, errors)
            self.assertIn(".github/actions/setup-rust/action.yml:", errors[0])
            self.assertIn("full 40-hex commit SHA", errors[0])

    def test_rejects_unverified_download(self) -> None:
        errors = self.errors("reject-download")
        self.assertTrue(any("SHA-256 verification" in error for error in errors))

    def test_rejects_mutable_compose_image(self) -> None:
        errors = self.errors("reject-compose-image")
        self.assertTrue(any("@sha256: digest" in error for error in errors))

    def test_rejects_floating_npm_channel(self) -> None:
        errors = self.errors("reject-npm-channel")
        self.assertTrue(any("floating npm/channel version" in error for error in errors))

    def test_rejects_unlocked_install(self) -> None:
        errors = self.errors("reject-unlocked-install")
        self.assertTrue(any("lockfile-safe mode" in error for error in errors))


class DashboardEmbedTests(unittest.TestCase):
    def workflow(self, name: str) -> str:
        return (ROOT.parent / ".github" / "workflows" / name).read_text(encoding="utf-8")

    def test_accepts_the_shipped_workflows(self) -> None:
        for name in ("build-native.yml", "publish.yml"):
            self.assertEqual(CHECKER.dashboard_embed_errors(self.workflow(name), name), [])

    def test_rejects_a_build_row_that_skips_the_hand_off(self) -> None:
        text = self.workflow("publish.yml").replace("CANON_DASHBOARD_DIST:", "UNRELATED:")
        errors = CHECKER.dashboard_embed_errors(text, "publish.yml")
        self.assertTrue(any("CANON_DASHBOARD_DIST" in error for error in errors), errors)

    def test_rejects_a_build_job_that_does_not_wait_for_the_app(self) -> None:
        text = self.workflow("build-native.yml").replace("needs: [safety, dashboard]", "needs: safety")
        errors = CHECKER.dashboard_embed_errors(text, "build-native.yml")
        self.assertTrue(any("needs" in error for error in errors), errors)

    def test_rejects_a_second_build_of_the_app(self) -> None:
        text = self.workflow("publish.yml") + "      - run: bun run --cwd packages/dashboard build\n"
        errors = CHECKER.dashboard_embed_errors(text, "publish.yml")
        self.assertTrue(any("exactly one job" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
