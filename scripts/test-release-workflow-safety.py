#!/usr/bin/env python3
"""Offline fixture tests for supply-chain checks in the release checker."""

from __future__ import annotations

import importlib.util
import pathlib
import subprocess
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


class PrereleasePublishTests(unittest.TestCase):
    """0.35.0-rc.1 publishes under `next`; 0.35.0 publishes as it always did."""

    def workflow(self) -> str:
        return (ROOT.parent / ".github" / "workflows" / "publish.yml").read_text(encoding="utf-8")

    def step_script(self, step_name: str) -> str:
        """The `run: |` body of one step of publish.yml, dedented."""
        step = CHECKER._step_block(self.workflow(), step_name)
        self.assertTrue(step, f"no step named {step_name!r}")
        lines = step.splitlines()
        start = next(i for i, line in enumerate(lines) if line.strip() == "run: |") + 1
        indent = len(lines[start]) - len(lines[start].lstrip())
        body: list[str] = []
        for line in lines[start:]:
            if line.strip() and len(line) - len(line.lstrip()) < indent:
                break
            body.append(line[indent:])
        return "\n".join(body)

    def publish_step_names(self) -> list[str]:
        job = CHECKER._job_block(self.workflow(), "publish")
        names = [
            step.splitlines()[0].strip().removeprefix("- name: ")
            for step in CHECKER._steps(job)
            if "npm publish" in step
        ]
        self.assertEqual(len(names), 4, names)
        return names

    def run_publish_steps(self, version: str) -> list[str]:
        """What each of the four publish steps runs `npm` with, for a version
        nothing is published at yet (a fake `npm view` fails, `npm publish` is logged)."""
        with tempfile.TemporaryDirectory() as directory:
            tmp = pathlib.Path(directory)
            bin_dir = tmp / "bin"
            bin_dir.mkdir()
            log = tmp / "npm.log"
            (bin_dir / "npm").write_text(
                '#!/usr/bin/env bash\n'
                'if [ "$1" = view ]; then exit 1; fi\n'
                'echo "$@" >> "$NPM_LOG"\n',
                encoding="utf-8",
            )
            (bin_dir / "jq").write_text('#!/usr/bin/env bash\necho pkg\n', encoding="utf-8")
            for tool in ("npm", "jq"):
                (bin_dir / tool).chmod(0o755)
            env = {
                "PATH": f"{bin_dir}:/usr/bin:/bin",
                "NPM_LOG": str(log),
                "VERSION": version,
            }
            for name in self.publish_step_names():
                script = tmp / "step.sh"
                script.write_text(self.step_script(name), encoding="utf-8")
                subprocess.run(["bash", str(script)], cwd=tmp, env=env, check=True)
            return log.read_text(encoding="utf-8").splitlines()

    def test_prerelease_publishes_every_package_under_next(self) -> None:
        self.assertEqual(
            self.run_publish_steps("0.35.0-rc.1"),
            ["publish --provenance --access public --tag next"] * 4,
        )

    def test_stable_publishes_every_package_with_no_tag(self) -> None:
        self.assertEqual(
            self.run_publish_steps("0.35.0"),
            ["publish --provenance --access public"] * 4,
        )

    def tag_check(self, tag: str, version: str) -> int:
        script = self.step_script("Verify tag matches workspace version (tag-triggered runs only)")
        script = script.replace("${{ github.ref_name }}", tag).replace(
            "${{ steps.read.outputs.version }}", version
        )
        return subprocess.run(
            ["bash", "-c", script], capture_output=True, text=True, check=False
        ).returncode

    def test_tag_check_accepts_a_prerelease_name_and_refuses_a_mismatch(self) -> None:
        self.assertEqual(self.tag_check("v0.35.0-rc.1", "0.35.0-rc.1"), 0)
        self.assertEqual(self.tag_check("v0.35.0", "0.35.0"), 0)
        self.assertEqual(self.tag_check("v0.35.0", "0.35.0-rc.1"), 1)
        self.assertEqual(self.tag_check("v0.35.0-rc.2", "0.35.0-rc.1"), 1)

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(CHECKER.prerelease_publish_errors(self.workflow(), "publish.yml"), [])

    def test_rejects_a_workflow_without_the_prerelease_rule(self) -> None:
        text = self.workflow()
        stripped = text.replace(
            '          elif [[ "$VERSION" == *-* ]]; then\n'
            "            npm publish --provenance --access public --tag next\n",
            "",
        )
        self.assertNotEqual(stripped, text)
        errors = CHECKER.prerelease_publish_errors(stripped, "publish.yml")
        self.assertEqual(len(errors), 4, errors)
        self.assertTrue(all("--tag next" in error for error in errors), errors)

    def test_rejects_a_single_publish_step_without_the_rule(self) -> None:
        text = self.workflow().replace(
            '          elif [[ "$VERSION" == *-* ]]; then\n'
            "            npm publish --provenance --access public --tag next\n",
            "",
            1,
        )
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertEqual(len(errors), 1, errors)

    def test_rejects_publishing_a_prerelease_under_latest(self) -> None:
        text = self.workflow().replace("--access public --tag next", "--access public --tag latest", 1)
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(any("--tag latest" in error for error in errors), errors)

    def test_rejects_a_prerelease_branch_that_is_not_guarded_by_the_hyphen(self) -> None:
        text = self.workflow().replace('[[ "$VERSION" == *-* ]]', '[[ "$VERSION" == *rc* ]]', 1)
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(any("hyphenated" in error for error in errors), errors)

    def test_rejects_a_stable_publish_that_also_runs_for_a_prerelease(self) -> None:
        text = self.workflow().replace(
            "          else\n            npm publish --provenance --access public\n",
            "          fi\n          npm publish --provenance --access public\n          if false; then\n            :\n",
            1,
        )
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(errors, errors)

    def test_rejects_a_publish_step_that_drops_the_version_env(self) -> None:
        text = self.workflow().replace(
            "          VERSION: ${{ needs.read-version.outputs.version }}\n          NODE_AUTH_TOKEN",
            "          NODE_AUTH_TOKEN",
            1,
        )
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(any("VERSION" in error for error in errors), errors)

    def test_a_github_release_must_be_marked_prerelease(self) -> None:
        release = "      - name: Create GitHub release\n        run: gh release create \"$GITHUB_REF_NAME\" --generate-notes\n"
        errors = CHECKER.prerelease_publish_errors(self.workflow() + release, "publish.yml")
        self.assertTrue(any("prerelease" in error for error in errors), errors)
        marked = (
            "      - name: Create GitHub release\n"
            "        run: |\n"
            '          if [[ "$GITHUB_REF_NAME" == *-* ]]; then PRE=--prerelease; fi\n'
            '          gh release create "$GITHUB_REF_NAME" $PRE\n'
        )
        self.assertEqual(CHECKER.prerelease_publish_errors(self.workflow() + marked, "publish.yml"), [])


if __name__ == "__main__":
    unittest.main()
