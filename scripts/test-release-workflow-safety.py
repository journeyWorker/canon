#!/usr/bin/env python3
"""Offline fixture tests for supply-chain checks in the release checker."""

from __future__ import annotations

import importlib.util
import json
import re
import sys
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

    def test_rejects_mutable_action_ref(self) -> None:
        errors = self.errors("reject-action-ref")
        self.assertTrue(errors)

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

    def test_rejects_unverified_download(self) -> None:
        errors = self.errors("reject-download")
        self.assertTrue(errors)

    def test_rejects_mutable_compose_image(self) -> None:
        errors = self.errors("reject-compose-image")
        self.assertTrue(errors)

    def test_rejects_floating_npm_channel(self) -> None:
        errors = self.errors("reject-npm-channel")
        self.assertTrue(errors)

    def test_rejects_unlocked_install(self) -> None:
        errors = self.errors("reject-unlocked-install")
        self.assertTrue(errors)


class DashboardEmbedTests(unittest.TestCase):
    def workflow(self, name: str) -> str:
        return (ROOT.parent / ".github" / "workflows" / name).read_text(encoding="utf-8")

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(CHECKER.dashboard_embed_errors(self.workflow("build-native.yml"), "build-native.yml"), [])

    def test_rejects_a_build_row_that_skips_the_hand_off(self) -> None:
        text = self.workflow("build-native.yml").replace("CANON_DASHBOARD_DIST:", "UNRELATED:")
        errors = CHECKER.dashboard_embed_errors(text, "build-native.yml")
        self.assertTrue(any("CANON_DASHBOARD_DIST" in error for error in errors), errors)

    def test_rejects_a_build_job_that_does_not_wait_for_the_app(self) -> None:
        text = self.workflow("build-native.yml").replace("needs: [safety, scope, dashboard]", "needs: [safety, scope]")
        errors = CHECKER.dashboard_embed_errors(text, "build-native.yml")
        self.assertTrue(errors)

    def test_rejects_a_second_build_of_the_app(self) -> None:
        text = self.workflow("build-native.yml") + "      - run: bun run --cwd packages/dashboard build\n"
        errors = CHECKER.dashboard_embed_errors(text, "build-native.yml")
        self.assertTrue(errors)


def workflow(name: str) -> str:
    return (ROOT.parent / ".github" / "workflows" / name).read_text(encoding="utf-8")


def repo_file(relative: str) -> str:
    return (ROOT.parent / relative).read_text(encoding="utf-8")


def mutate(text: str, old: str, new: str, count: int = -1) -> str:
    """`text` with `old` replaced, and a failure when `old` is not there: a test that mutates nothing proves nothing."""
    if old not in text:
        raise AssertionError(f"the test's mutation target is not in the text: {old!r}")
    return text.replace(old, new, count)


def alpine_image(name: str) -> str:
    """The aarch64 musl row's pinned image, as the workflow writes it: the tests mutate it and never spell a version."""
    found = re.search(r"rust:\S+@sha256:[0-9a-f]+", workflow(name))
    if not found:
        raise AssertionError(f"{name} has no pinned rust image")
    return found.group(0)


class PrereleasePublishTests(unittest.TestCase):
    """0.35.0-rc.1 publishes under `next`; 0.35.0 publishes as it always did. Since 0.42 every package is a tarball
    packed once (`npm publish "$TARBALL"`), so the commands carry its path."""

    def workflow(self) -> str:
        return workflow("publish.yml")

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

    def publish_steps(self) -> list[tuple[str, str]]:
        """(step name, PACKAGE env) of the publish steps, in order."""
        job = CHECKER._job_block(self.workflow(), "publish")
        found = []
        for step in CHECKER._steps(job):
            if "npm publish" in step:
                package = re.search(r'PACKAGE: "([^"]+)"', step)
                self.assertTrue(package, step)
                found.append((step.splitlines()[0].strip().removeprefix("- name: "), package.group(1)))
        self.assertEqual(len(found), 5, found)
        return found

    def run_publish_steps(self, version: str) -> list[str]:
        """What each of the five publish steps runs `npm` with, for a version nothing is published at yet
        (a fake `npm view` fails, `npm publish` is logged) and the tarballs `package` would have left in tarballs/."""
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
            (bin_dir / "npm").chmod(0o755)
            (tmp / "tarballs").mkdir()
            for name, package in self.publish_steps():
                (tmp / "tarballs" / f"{package.removeprefix('@').replace('/', '-')}-{version}.tgz").write_text("tarball")
                script = tmp / "step.sh"
                script.write_text(self.step_script(name), encoding="utf-8")
                env = {"PATH": f"{bin_dir}:/usr/bin:/bin", "NPM_LOG": str(log), "VERSION": version, "PACKAGE": package}
                subprocess.run(["bash", str(script)], cwd=tmp, env=env, check=True)
            return log.read_text(encoding="utf-8").splitlines()

    def expected(self, version: str, tag: str) -> list[str]:
        return [
            f"publish ./tarballs/{package.removeprefix('@').replace('/', '-')}-{version}.tgz --provenance --access public{tag}"
            for _, package in self.publish_steps()
        ]

    def test_prerelease_publishes_every_package_under_next(self) -> None:
        self.assertEqual(self.run_publish_steps("0.35.0-rc.1"), self.expected("0.35.0-rc.1", " --tag next"))

    def test_stable_publishes_every_package_with_no_tag(self) -> None:
        self.assertEqual(self.run_publish_steps("0.35.0"), self.expected("0.35.0", ""))

    def test_the_publish_steps_publish_the_packed_tarball_and_never_a_directory(self) -> None:
        job = CHECKER._job_block(self.workflow(), "publish")
        self.assertNotIn("working-directory", job)
        text = mutate(self.workflow(), 'npm publish "$TARBALL" --provenance --access public --tag next', 'npm publish --provenance --access public --tag next', 1)
        self.assertTrue(CHECKER.smoke_errors(text, workflow("build-native.yml"), {}))
        self.assertTrue(CHECKER.prerelease_publish_errors(text, "publish.yml"))

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

    PRE_BRANCH = (
        '          elif [[ "$VERSION" == *-* ]]; then\n'
        '            npm publish "$TARBALL" --provenance --access public --tag next\n'
    )

    def test_rejects_a_workflow_without_the_prerelease_rule(self) -> None:
        stripped = mutate(self.workflow(), self.PRE_BRANCH, "")
        errors = CHECKER.prerelease_publish_errors(stripped, "publish.yml")
        self.assertEqual(len(errors), 5, errors)

    def test_rejects_a_single_publish_step_without_the_rule(self) -> None:
        text = mutate(self.workflow(), self.PRE_BRANCH, "", 1)
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertEqual(len(errors), 1, errors)

    def test_rejects_publishing_a_prerelease_under_latest(self) -> None:
        text = mutate(self.workflow(), "--access public --tag next", "--access public --tag latest", 1)
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(any("--tag latest" in error for error in errors), errors)

    def test_rejects_a_prerelease_branch_that_is_not_guarded_by_the_hyphen(self) -> None:
        text = mutate(self.workflow(), '[[ "$VERSION" == *-* ]]', '[[ "$VERSION" == *rc* ]]', 1)
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(errors)

    def test_rejects_a_stable_publish_that_also_runs_for_a_prerelease(self) -> None:
        text = mutate(
            self.workflow(),
            '          else\n            npm publish "$TARBALL" --provenance --access public\n',
            '          fi\n          npm publish "$TARBALL" --provenance --access public\n          if false; then\n            :\n',
            1,
        )
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(errors, errors)

    def test_rejects_a_publish_step_that_drops_the_version_env(self) -> None:
        text = mutate(
            self.workflow(),
            '          VERSION: ${{ needs.read-version.outputs.version }}\n          PACKAGE:',
            "          PACKAGE:",
            1,
        )
        errors = CHECKER.prerelease_publish_errors(text, "publish.yml")
        self.assertTrue(any("VERSION" in error for error in errors), errors)

    def test_a_github_release_must_be_marked_prerelease(self) -> None:
        release = "      - name: Create GitHub release\n        run: gh release create \"$GITHUB_REF_NAME\" --generate-notes\n"
        errors = CHECKER.prerelease_publish_errors(self.workflow() + release, "publish.yml")
        self.assertTrue(errors)
        marked = (
            "      - name: Create GitHub release\n"
            "        run: |\n"
            '          if [[ "$GITHUB_REF_NAME" == *-* ]]; then PRE=--prerelease; fi\n'
            '          gh release create "$GITHUB_REF_NAME" $PRE\n'
        )
        self.assertEqual(CHECKER.prerelease_publish_errors(self.workflow() + marked, "publish.yml"), [])


class RunnerLabelTests(unittest.TestCase):
    def test_accepts_the_shipped_workflows(self) -> None:
        for name in ("build-native.yml", "publish.yml"):
            self.assertEqual(CHECKER.runner_label_errors(workflow(name), name), [])

    def test_rejects_a_latest_label_in_a_job(self) -> None:
        text = mutate(workflow("publish.yml"), "    runs-on: ubuntu-26.04\n", "    runs-on: ubuntu-latest\n", 1)
        errors = CHECKER.runner_label_errors(text, "publish.yml")
        self.assertTrue(any("ubuntu-latest" in error for error in errors), errors)

    def test_rejects_a_latest_label_in_a_matrix_row(self) -> None:
        text = mutate(workflow("build-native.yml"), "host: windows-2025", "host: windows-latest")
        errors = CHECKER.runner_label_errors(text, "build-native.yml")
        self.assertTrue(any("windows-latest" in error for error in errors), errors)
        text = mutate(workflow("publish.yml"), "runner: ubuntu-26.04-arm", "runner: ubuntu-latest", 1)
        self.assertTrue(CHECKER.runner_label_errors(text, "publish.yml"))

    def test_rejects_a_pinned_label_that_is_not_the_allowed_one(self) -> None:
        text = mutate(workflow("build-native.yml"), "host: macos-15", "host: macos-14")
        errors = CHECKER.runner_label_errors(text, "build-native.yml")
        self.assertTrue(any("macos-14" in error for error in errors), errors)

    def test_rejects_a_job_without_runs_on(self) -> None:
        text = mutate(workflow("build-native.yml"), "  safety:\n    name: Release workflow safety check\n    runs-on: ubuntu-26.04\n", "  safety:\n    name: Release workflow safety check\n")
        self.assertTrue(CHECKER.runner_label_errors(text, "build-native.yml"))


class BuildRowTests(unittest.TestCase):
    NAME = "build-native.yml"
    X64_ROW = '            package_dir: core-linux-x64\n            binary_ext: ""\n'

    def rows(self) -> list[dict[str, str]]:
        return CHECKER.matrix_rows(CHECKER._job_block(workflow(self.NAME), "build"))

    def test_accepts_the_shipped_rows(self) -> None:
        self.assertEqual(CHECKER.build_row_errors(self.rows(), self.NAME), [])
        self.assertEqual(len(self.rows()), 4)

    def broken(self, old: str, new: str) -> list[str]:
        text = mutate(workflow(self.NAME), old, new)
        return CHECKER.build_row_errors(CHECKER.matrix_rows(CHECKER._job_block(text, "build")), self.NAME)

    def test_rejects_a_linux_row_that_builds_gnu(self) -> None:
        errors = self.broken("target: x86_64-unknown-linux-musl", "target: x86_64-unknown-linux-gnu")
        self.assertTrue(errors)

    def test_rejects_a_linux_row_on_the_wrong_architecture(self) -> None:
        errors = self.broken("host: ubuntu-26.04-arm\n            target: aarch64-unknown-linux-musl", "host: ubuntu-26.04\n            target: aarch64-unknown-linux-musl")
        self.assertTrue(errors)

    def test_rejects_a_missing_row(self) -> None:
        errors = self.broken("target: aarch64-unknown-linux-musl", "target: riscv64gc-unknown-linux-musl")
        self.assertTrue(errors)

    def test_the_aarch64_musl_row_must_have_the_64k_page_size(self) -> None:
        errors = self.broken('            lg_page: "16"\n', "")
        self.assertTrue(errors)
        errors = self.broken('lg_page: "16"', 'lg_page: "14"')
        self.assertTrue(errors)

    def test_the_x86_64_row_must_not_have_a_page_size(self) -> None:
        errors = self.broken(self.X64_ROW, self.X64_ROW + '            lg_page: "16"\n')
        self.assertTrue(errors)

    def test_the_aarch64_musl_row_must_name_an_alpine_image(self) -> None:
        image = f"            alpine_image: {alpine_image(self.NAME)}\n"
        errors = self.broken(image, "")
        self.assertTrue(errors)
        errors = self.broken(f"alpine_image: {alpine_image(self.NAME).split('@')[0]}@sha256:", "alpine_image: ubuntu:26.04@sha256:")
        self.assertTrue(errors)

    def test_the_x86_64_row_must_not_have_an_alpine_image(self) -> None:
        errors = self.broken(self.X64_ROW, self.X64_ROW + f"            alpine_image: {alpine_image(self.NAME)}\n")
        self.assertTrue(errors)

    def test_the_alpine_image_is_pinned_by_digest(self) -> None:
        image = alpine_image(self.NAME)
        text = mutate(workflow(self.NAME), image, image.split("@")[0])
        errors = CHECKER.image_digest_errors(text, self.NAME)
        self.assertTrue(errors)

    def test_macos_and_windows_are_one_arch_each(self) -> None:
        errors = self.broken("target: aarch64-apple-darwin", "target: x86_64-apple-darwin")
        self.assertTrue(errors)
        errors = self.broken("target: x86_64-pc-windows-msvc", "target: aarch64-pc-windows-msvc")
        self.assertTrue(errors)


class AlpineBuildTests(unittest.TestCase):
    NAME = "build-native.yml"

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(CHECKER.alpine_build_errors(workflow(self.NAME), self.NAME), [])

    def test_rejects_a_step_that_does_not_run_the_rows_image(self) -> None:
        text = mutate(workflow(self.NAME), '"$ALPINE_IMAGE" sh -s', "alpine:3.24 sh -s")
        errors = CHECKER.alpine_build_errors(text, self.NAME)
        self.assertTrue(errors)

    def test_rejects_a_step_that_does_not_mount_the_workspace(self) -> None:
        text = mutate(workflow(self.NAME), '-v "$GITHUB_WORKSPACE:$GITHUB_WORKSPACE" -w "$GITHUB_WORKSPACE" \\\n', "")
        self.assertTrue(CHECKER.alpine_build_errors(text, self.NAME))

    def test_rejects_a_step_that_loses_the_page_size_or_the_dashboard(self) -> None:
        text = mutate(workflow(self.NAME), "-e CANON_DASHBOARD_DIST -e JEMALLOC_SYS_WITH_LG_PAGE ", "")
        errors = CHECKER.alpine_build_errors(text, self.NAME)
        self.assertTrue(errors)

    def test_rejects_a_step_that_does_not_compare_the_toolchain(self) -> None:
        text = mutate(workflow(self.NAME), '[ "$got" != "$want" ]', "false")
        self.assertTrue(CHECKER.alpine_build_errors(text, self.NAME))

    def test_rejects_a_host_toolchain_install_on_the_alpine_row(self) -> None:
        text = mutate(workflow(self.NAME), "        if: matrix.settings.alpine_image == ''\n", "")
        self.assertTrue(CHECKER.alpine_build_errors(text, self.NAME))


class JemallocTests(unittest.TestCase):
    NAME = "build-native.yml"

    def check(self, text: str, cargo: str | None = None, main: str | None = None) -> list[str]:
        return CHECKER.jemalloc_errors(
            text,
            self.NAME,
            cargo if cargo is not None else repo_file("crates/canon-cli/Cargo.toml"),
            main if main is not None else repo_file("crates/canon-cli/src/main.rs"),
        )

    def test_accepts_the_shipped_files(self) -> None:
        self.assertEqual(self.check(workflow(self.NAME)), [])

    def test_rejects_the_dependency_outside_the_musl_table(self) -> None:
        cargo = repo_file("crates/canon-cli/Cargo.toml")
        moved = mutate(cargo, "[target.'cfg(target_env = \"musl\")'.dependencies]\ntikv-jemallocator = \"0.6\"\n", "")
        moved = mutate(moved, "[build-dependencies]", "[build-dependencies]\n").replace("[dependencies]\n", "[dependencies]\ntikv-jemallocator = \"0.6\"\n", 1)
        errors = self.check(workflow(self.NAME), cargo=moved)
        self.assertTrue(errors)

    def test_rejects_the_allocator_on_every_target(self) -> None:
        main = mutate(repo_file("crates/canon-cli/src/main.rs"), '#[cfg(target_env = "musl")]\n#[global_allocator]', "#[global_allocator]")
        errors = self.check(workflow(self.NAME), main=main)
        self.assertTrue(errors)

    def test_rejects_a_build_step_that_does_not_pass_the_page_size(self) -> None:
        text = mutate(workflow(self.NAME), "JEMALLOC_SYS_WITH_LG_PAGE: ${{ matrix.settings.lg_page }}", "UNRELATED: 1")
        errors = self.check(text)
        self.assertTrue(errors)

    def test_rejects_a_hardcoded_page_size(self) -> None:
        text = workflow(self.NAME) + "      - run: JEMALLOC_SYS_WITH_LG_PAGE=16 cargo build\n"
        errors = self.check(text)
        self.assertTrue(errors)


class MacosTargetTests(unittest.TestCase):
    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(CHECKER.macos_target_errors(CHECKER.top_level_env(workflow("build-native.yml")), "build-native.yml"), [])

    def test_rejects_another_value_or_none(self) -> None:
        text = mutate(workflow("build-native.yml"), 'MACOSX_DEPLOYMENT_TARGET: "15.0"', 'MACOSX_DEPLOYMENT_TARGET: "14.0"')
        errors = CHECKER.macos_target_errors(CHECKER.top_level_env(text), "build-native.yml")
        self.assertTrue(errors)
        text = mutate(workflow("build-native.yml"), '  MACOSX_DEPLOYMENT_TARGET: "15.0"\n', "")
        self.assertTrue(CHECKER.macos_target_errors(CHECKER.top_level_env(text), "build-native.yml"))

    def test_the_shipped_files_pass_the_checker(self) -> None:
        result = subprocess.run([sys.executable, str(ROOT / "check-release-workflow-safety.py")], capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("MACOSX_DEPLOYMENT_TARGET", CHECKER.REQUIRED_ENV)


class CrtStaticTests(unittest.TestCase):
    def workflows(self) -> dict[str, str]:
        return {name: workflow(name) for name in ("build-native.yml", "publish.yml", "test.yml", "pr-guard.yml")}

    def test_accepts_the_shipped_files(self) -> None:
        self.assertEqual(CHECKER.crt_static_errors(repo_file(".cargo/config.toml"), self.workflows()), [])

    def test_rejects_a_config_without_crt_static(self) -> None:
        config = mutate(repo_file(".cargo/config.toml"), "target-feature=+crt-static", "target-feature=+avx2")
        self.assertTrue(CHECKER.crt_static_errors(config, self.workflows()))
        self.assertTrue(CHECKER.crt_static_errors("[alias]\nxtask = \"x\"\n", self.workflows()))

    def test_rejects_a_workflow_rustflags(self) -> None:
        workflows = self.workflows()
        workflows["build-native.yml"] = mutate(workflows["build-native.yml"], '  CARGO_INCREMENTAL: 0\n', '  CARGO_INCREMENTAL: 0\n  RUSTFLAGS: "-C target-cpu=native"\n', 1)
        errors = CHECKER.crt_static_errors(repo_file(".cargo/config.toml"), workflows)
        self.assertTrue(errors)
        workflows["publish.yml"] = workflow("publish.yml") + "      - run: CARGO_BUILD_RUSTFLAGS=-Cx cargo build\n"
        self.assertTrue(CHECKER.crt_static_errors(repo_file(".cargo/config.toml"), workflows))


class BinaryCheckStepTests(unittest.TestCase):
    def rows(self, name: str) -> list[dict[str, str]]:
        return CHECKER.matrix_rows(CHECKER._job_block(workflow(name), "build"))

    def check(self, name: str, text: str | None = None, checker: str | None = None) -> list[str]:
        return CHECKER.binary_check_errors(
            text if text is not None else workflow(name),
            name,
            self.rows(name),
            checker if checker is not None else repo_file("scripts/check-release-binary.py"),
        )

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check("build-native.yml"), [])

    def test_rejects_a_build_job_without_the_step(self) -> None:
        text = mutate(workflow("build-native.yml"), CHECKER.BINARY_CHECK_STEP, "Something else")
        self.assertTrue(self.check("build-native.yml", text))

    def test_rejects_a_step_that_does_not_name_the_row(self) -> None:
        text = mutate(workflow("build-native.yml"), '--target "${{ matrix.settings.target }}" "target', '"target')
        self.assertTrue(self.check("build-native.yml", text))

    def test_rejects_a_step_that_skips_a_row(self) -> None:
        text = mutate(workflow("build-native.yml"), f"      - name: {CHECKER.BINARY_CHECK_STEP}\n", f"      - name: {CHECKER.BINARY_CHECK_STEP}\n        if: matrix.settings.binary_ext == ''\n")
        self.assertTrue(self.check("build-native.yml", text))

    def test_rejects_a_target_the_script_does_not_know(self) -> None:
        checker = mutate(repo_file("scripts/check-release-binary.py"), '"aarch64-apple-darwin": {"format"', '"aarch64-apple-ios": {"format"')
        self.assertTrue(self.check("build-native.yml", checker=checker))

    def test_rejects_a_container_build(self) -> None:
        text = mutate(workflow("build-native.yml"), "    timeout-minutes: 45\n    strategy:", "    timeout-minutes: 45\n    container: alpine:3.24\n    strategy:")
        self.assertTrue(self.check("build-native.yml", text))


class LauncherTests(unittest.TestCase):
    def rows(self) -> list[dict[str, str]]:
        return CHECKER.matrix_rows(CHECKER._job_block(workflow("build-native.yml"), "build"))

    def manifests(self) -> dict[str, dict]:
        return {p.name: json.loads((p / "package.json").read_text()) for p in sorted((ROOT.parent / "packages").glob("core-*"))}

    def test_accepts_the_shipped_files(self) -> None:
        self.assertEqual(CHECKER.launcher_errors(repo_file("packages/cli/src/index.ts"), self.rows(), self.manifests()), [])
        self.assertEqual(len(self.manifests()), 4)

    def test_rejects_libc_detection_in_the_launcher(self) -> None:
        for needle in ("CANON_LIBC", "detectLibcKind"):
            text = repo_file("packages/cli/src/index.ts") + f"\n// {needle}\n"
            errors = CHECKER.launcher_errors(text, self.rows(), self.manifests())
            self.assertTrue(any(needle in error for error in errors), errors)

    def test_rejects_a_package_the_launcher_cannot_resolve(self) -> None:
        text = mutate(repo_file("packages/cli/src/index.ts"), 'if (arch === "arm64") return "core-linux-arm64";\n', "")
        errors = CHECKER.launcher_errors(text, self.rows(), self.manifests())
        self.assertTrue(any("core-linux-arm64" in error for error in errors), errors)

    def test_rejects_a_package_no_row_ships(self) -> None:
        text = mutate(repo_file("packages/cli/src/index.ts"), 'if (arch === "arm64") return "core-linux-arm64";\n', 'if (arch === "arm64") return "core-linux-arm64";\n    if (arch === "riscv64") return "core-linux-riscv64";\n')
        errors = CHECKER.launcher_errors(text, self.rows(), self.manifests())
        self.assertTrue(any("core-linux-riscv64" in error for error in errors), errors)

    def test_rejects_a_libc_filter_and_wrong_os_cpu_in_a_manifest(self) -> None:
        manifests = self.manifests()
        manifests["core-linux-x64"]["libc"] = ["glibc"]
        errors = CHECKER.launcher_errors(repo_file("packages/cli/src/index.ts"), self.rows(), manifests)
        self.assertTrue(errors)
        manifests = self.manifests()
        manifests["core-linux-arm64"]["cpu"] = ["x64"]
        errors = CHECKER.launcher_errors(repo_file("packages/cli/src/index.ts"), self.rows(), manifests)
        self.assertTrue(errors)


class PackageSetTests(unittest.TestCase):
    def rows(self) -> list[dict[str, str]]:
        return CHECKER.matrix_rows(CHECKER._job_block(workflow("build-native.yml"), "build"))

    def names(self) -> dict[str, str]:
        return {p.name: json.loads((p / "package.json").read_text())["name"] for p in sorted((ROOT.parent / "packages").glob("core-*"))}

    def check(self, text: str | None = None, bun_lock: str | None = None, optional: set[str] | None = None) -> list[str]:
        cli = json.loads(repo_file("packages/cli/package.json"))
        return CHECKER.package_set_errors(
            text if text is not None else workflow("publish.yml"),
            self.rows(),
            self.names(),
            cli["name"],
            bun_lock if bun_lock is not None else repo_file("bun.lock"),
            optional if optional is not None else set(cli["optionalDependencies"]),
        )

    def test_accepts_the_shipped_files(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_missing_download(self) -> None:
        text = mutate(workflow("publish.yml"), "          name: canon-binary-aarch64-unknown-linux-musl\n          path: packages/core-linux-arm64/bin\n", "          name: unrelated\n          path: elsewhere\n")
        errors = self.check(text)
        self.assertTrue(any("core-linux-arm64" in error for error in errors), errors)

    def test_rejects_a_download_of_the_wrong_binary(self) -> None:
        text = mutate(workflow("publish.yml"), "name: canon-binary-aarch64-unknown-linux-musl\n          path: packages/core-linux-arm64/bin", "name: canon-binary-x86_64-unknown-linux-musl\n          path: packages/core-linux-arm64/bin")
        self.assertTrue(any("canon-binary-x86_64-unknown-linux-musl" in error for error in self.check(text)))

    def test_rejects_a_package_missing_from_each_list(self) -> None:
        publish = workflow("publish.yml")
        cases = {
            "checksum verification": ("          (cd packages/core-linux-arm64/bin && sha256sum -c canon.sha256)\n", ""),
            "executable bit": ("          chmod +x packages/core-linux-arm64/bin/canon\n", ""),
            "version application": ("packages/core-darwin-arm64 packages/core-linux-arm64 packages/core-linux-x64 packages/core-win32-x64; do\n            jq", "packages/core-darwin-arm64 packages/core-linux-x64 packages/core-win32-x64; do\n            jq"),
            "npm pack": ("packages/core-linux-arm64 packages/core-linux-x64 packages/core-win32-x64 packages/cli; do\n            npm pack", "packages/core-linux-x64 packages/core-win32-x64 packages/cli; do\n            npm pack"),
            "pre-flight": ("for pkg_dir in core-linux-arm64 core-darwin-arm64", "for pkg_dir in core-darwin-arm64"),
        }
        for place, (old, new) in cases.items():
            errors = self.check(mutate(publish, old, new))
            self.assertTrue(any("core-linux-arm64" in error for error in errors), (place, errors))

    def test_rejects_an_unpublished_package(self) -> None:
        text = mutate(workflow("publish.yml"), 'PACKAGE: "@journeykit/canon-core-win32-x64"', 'PACKAGE: "@journeykit/canon-core-linux-x64"')
        errors = self.check(text)
        self.assertTrue(any("npm publish steps" in error and "core-win32-x64" in error for error in errors), errors)

    def test_rejects_a_publish_order_that_does_not_start_with_arm64_and_end_with_the_launcher(self) -> None:
        publish = workflow("publish.yml")
        arm = 'PACKAGE: "@journeykit/canon-core-linux-arm64"'
        darwin = 'PACKAGE: "@journeykit/canon-core-darwin-arm64"'
        swapped = mutate(mutate(mutate(publish, arm, "PACKAGE: TMP"), darwin, arm), "PACKAGE: TMP", darwin)
        self.assertTrue(self.check(swapped))
        launcher = 'PACKAGE: "@journeykit/canon"'
        win = 'PACKAGE: "@journeykit/canon-core-win32-x64"'
        swapped = mutate(mutate(mutate(publish, launcher, "PACKAGE: TMP"), win, launcher), "PACKAGE: TMP", win)
        self.assertTrue(self.check(swapped))

    def test_rejects_an_optional_dependency_and_a_lockfile_without_the_package(self) -> None:
        cli = json.loads(repo_file("packages/cli/package.json"))
        optional = set(cli["optionalDependencies"]) - {"@journeykit/canon-core-linux-arm64"}
        self.assertTrue(any("linux-arm64" in error for error in self.check(optional=optional)))
        lock = mutate(repo_file("bun.lock"), '    "packages/core-linux-arm64": {', '    "packages/other": {')
        self.assertTrue(any("core-linux-arm64" in error for error in self.check(bun_lock=lock)))

    def test_rejects_a_second_pack_in_the_publish_job_and_a_directory_publish(self) -> None:
        publish = workflow("publish.yml")
        again = mutate(publish, "          set -euo pipefail\n          TARBALL=", "          set -euo pipefail\n          npm pack ./packages/cli\n          TARBALL=", 1)
        self.assertTrue(any("must not `npm pack` again" in error for error in CHECKER.smoke_errors(again, workflow("build-native.yml"), {})))
        self.assertEqual(CHECKER.smoke_errors(publish, workflow("build-native.yml"), {"test.yml": workflow("test.yml"), "pr-guard.yml": workflow("pr-guard.yml")}), [])


class ImageDigestTests(unittest.TestCase):
    def test_accepts_the_shipped_workflows(self) -> None:
        for name in ("build-native.yml", "publish.yml"):
            self.assertEqual(CHECKER.image_digest_errors(workflow(name), name), [])

    def test_rejects_a_smoke_image_without_a_digest(self) -> None:
        text = mutate(workflow("publish.yml"), "image: debian:12@sha256:268fb24c39bca92d326dd83ea3abebe4a3f7751c804ffef23bb6d857dd343ff8", "image: debian:12", 1)
        errors = CHECKER.image_digest_errors(text, "publish.yml")
        self.assertEqual(len(errors), 1, errors)

    def test_rejects_the_service_image_without_a_digest(self) -> None:
        text = mutate(workflow("publish.yml"), "POSTGRES_IMAGE: postgres:16-alpine@sha256:721873c34ceb9f8d8fc265984940dc982404c105f19ad51be9fdc5970a6080ea", "POSTGRES_IMAGE: postgres:16-alpine")
        self.assertTrue(any("postgres:16-alpine" in error for error in CHECKER.image_digest_errors(text, "publish.yml")))

    def test_rejects_a_container_without_a_digest(self) -> None:
        text = workflow("publish.yml") + "    container: node:26\n"
        self.assertTrue(CHECKER.image_digest_errors(text, "publish.yml"))

    def test_a_matrix_name_that_only_contains_image_is_not_an_image(self) -> None:
        self.assertEqual(CHECKER.image_digest_errors("            image_name: debian-12\n", "publish.yml"), [])


class SmokeTests(unittest.TestCase):
    OTHERS = ("test.yml", "pr-guard.yml")

    def check(self, publish: str | None = None, build: str | None = None, others: dict[str, str] | None = None) -> list[str]:
        return CHECKER.smoke_errors(
            publish if publish is not None else workflow("publish.yml"),
            build if build is not None else workflow("build-native.yml"),
            others if others is not None else {name: workflow(name) for name in self.OTHERS},
        )

    def test_accepts_the_shipped_workflows(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_missing_smoke_job(self) -> None:
        for job in CHECKER.SMOKE_JOBS:
            text = mutate(workflow("publish.yml"), f"  {job}:\n", f"  {job}-renamed:\n")
            errors = self.check(publish=text)
            self.assertTrue(any(job in error for error in errors), (job, errors))

    def test_rejects_a_publish_job_that_does_not_wait_for_a_smoke(self) -> None:
        for job in CHECKER.SMOKE_JOBS:
            text = mutate(workflow("publish.yml"), f"needs: [read-version, package, smoke-binary, smoke-install, smoke-network]", "needs: [read-version, package, " + ", ".join(j for j in CHECKER.SMOKE_JOBS if j != job) + "]")
            errors = self.check(publish=text)
            self.assertTrue(any(job in error for error in errors), (job, errors))
        text = mutate(workflow("publish.yml"), "needs: [read-version, package, smoke-binary, smoke-install, smoke-network]", "needs: [read-version, smoke-binary, smoke-install, smoke-network]")
        self.assertTrue(self.check(publish=text))

    def test_rejects_a_smoke_without_an_image_family_on_a_runner(self) -> None:
        text = mutate(workflow("publish.yml"), "image: alpine:3.24@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf", "image: debian:12@sha256:268fb24c39bca92d326dd83ea3abebe4a3f7751c804ffef23bb6d857dd343ff8", 1)
        errors = self.check(publish=text)
        self.assertTrue(errors)

    def test_rejects_a_smoke_in_build_native_or_any_other_workflow(self) -> None:
        build = workflow("build-native.yml") + "\n  smoke-binary:\n    runs-on: ubuntu-26.04\n    steps: []\n"
        self.assertTrue(self.check(build=build))
        others = {name: workflow(name) for name in self.OTHERS}
        others["test.yml"] += "      - run: scripts/release-smoke.sh binary\n"
        self.assertTrue(self.check(others=others))

    def test_rejects_a_publish_workflow_that_runs_on_pull_requests(self) -> None:
        text = mutate(workflow("publish.yml"), "on:\n  push:\n    tags:", "on:\n  pull_request:\n  push:\n    tags:")
        self.assertTrue(self.check(publish=text))

    def test_rejects_a_smoke_install_that_packs_its_own_tarballs(self) -> None:
        text = mutate(workflow("publish.yml"), "          name: npm-tarballs\n          path: tarballs\n\n      - name: Install the tarballs in the image", "          name: other\n          path: tarballs\n\n      - name: Install the tarballs in the image")
        self.assertTrue(self.check(publish=text))


class ConditionTests(unittest.TestCase):
    """The expression of a publish step's NPM_CONFIG_DRY_RUN, read the way Actions reads it and written as the string npm gets."""

    def renders(self, expression: str) -> list[str]:
        return [CHECKER._render(CHECKER._Cond(expression, context).parse()) for _, context, _ in CHECKER.DRY_RUN_SCENARIOS]

    def test_the_shipped_expression_reads_false_true_false(self) -> None:
        self.assertEqual(self.renders("github.event_name == 'workflow_dispatch' && inputs.dry_run"), ["false", "true", "false"])

    def test_a_form_that_reads_empty_on_a_tag_push_differs(self) -> None:
        self.assertNotEqual(self.renders("inputs.dry_run"), ["false", "true", "false"])
        self.assertNotEqual(self.renders("github.event.inputs.dry_run"), ["false", "true", "false"])

    def test_a_comparison_of_a_boolean_to_a_string_is_false(self) -> None:
        # Actions compares `true == 'true'` as 1 and NaN: not equal.
        self.assertNotEqual(self.renders("github.event_name == 'workflow_dispatch' && inputs.dry_run == 'true'"), ["false", "true", "false"])

    def test_equivalent_forms_read_the_same(self) -> None:
        for expression in (
            "github.event_name == 'workflow_dispatch' && github.event.inputs.dry_run == 'true'",
            "github.event_name == 'workflow_dispatch' && inputs.dry_run == true",
            "!(github.event_name != 'workflow_dispatch' || !inputs.dry_run)",
        ):
            self.assertEqual(self.renders(expression), ["false", "true", "false"], expression)

    def test_an_unknown_name_or_operator_is_an_error_not_a_pass(self) -> None:
        with self.assertRaises(ValueError):
            CHECKER._Cond("github.ref == 'x'", {}).parse()
        with self.assertRaises(ValueError):
            CHECKER._Cond("inputs.dry_run ~ 1", {"inputs.dry_run": True}).parse()


class DryRunTests(unittest.TestCase):
    """The five publish steps run their command in a dry run too: NPM_CONFIG_DRY_RUN on each, no `if:` on dry_run, no `--dry-run`."""

    ENV = "          NPM_CONFIG_DRY_RUN: ${{ github.event_name == 'workflow_dispatch' && inputs.dry_run }}\n"
    PUBLISH_IF = "        if: github.event_name != 'workflow_dispatch' || !inputs.dry_run\n"
    FIRST_STEP = "      - name: Publish @journeykit/canon-core-linux-arm64 (idempotent)\n"

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.dry_run_errors(text if text is not None else workflow("publish.yml"), "publish.yml")

    def with_env(self, value: str, count: int = -1) -> str:
        return mutate(workflow("publish.yml"), self.ENV, f"          NPM_CONFIG_DRY_RUN: {value}\n", count)

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_accepts_expressions_that_read_the_same(self) -> None:
        for value in (
            "${{ github.event_name == 'workflow_dispatch' && github.event.inputs.dry_run == 'true' }}",
            "${{ !(github.event_name != 'workflow_dispatch' || !inputs.dry_run) }}",
        ):
            self.assertEqual(self.check(self.with_env(value)), [], value)

    def test_rejects_a_dry_run_input_that_is_not_true_by_default(self) -> None:
        self.assertTrue(self.check(mutate(workflow("publish.yml"), "        default: true\n", "        default: false\n")))
        self.assertTrue(self.check(mutate(workflow("publish.yml"), "      dry_run:\n", "      dryrun:\n")))

    def test_rejects_a_publish_step_without_the_variable(self) -> None:
        self.assertTrue(self.check(mutate(workflow("publish.yml"), self.ENV, "", 1)))

    def test_rejects_a_value_that_is_not_false_true_false(self) -> None:
        for value in (
            '"true"',
            '"false"',
            "true",
            "${{ inputs.dry_run }}",
            "${{ github.event.inputs.dry_run }}",
            "${{ github.event_name == 'workflow_dispatch' && !inputs.dry_run }}",
            "${{ github.event_name == 'workflow_dispatch' && inputs.dry_run == 'true' }}",
            "${{ github.event_name != 'workflow_dispatch' || inputs.dry_run }}",
            "${{ github.ref_name }}",
        ):
            self.assertTrue(self.check(self.with_env(value, 1)), value)

    def test_rejects_the_variable_under_another_case_or_spelling_that_misreads(self) -> None:
        self.assertTrue(self.check(mutate(workflow("publish.yml"), self.ENV, "          npm_config_dry-run: true\n", 1)))
        self.assertTrue(self.check(mutate(workflow("publish.yml"), self.ENV, "          npm_config_dry_run: ${{ inputs.dry_run }}\n", 1)))

    def test_accepts_the_variable_in_another_case_that_reads_right(self) -> None:
        text = mutate(workflow("publish.yml"), self.ENV, self.ENV.replace("NPM_CONFIG_DRY_RUN", "npm_config_dry_run"), 1)
        self.assertEqual(self.check(text), [])

    def test_rejects_a_condition_on_a_publish_step(self) -> None:
        for condition in (self.PUBLISH_IF, "        if: ${{ !inputs.dry_run }}\n", "        if: github.event.inputs.dry_run != 'true'\n"):
            self.assertTrue(self.check(mutate(workflow("publish.yml"), self.FIRST_STEP, self.FIRST_STEP + condition)), condition)

    def test_rejects_a_condition_on_dry_run_on_any_other_step(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        self.assertTrue(self.check(mutate(workflow("publish.yml"), step, step + self.PUBLISH_IF)))

    def test_rejects_a_condition_on_a_job(self) -> None:
        text = mutate(workflow("publish.yml"), "  smoke-binary:\n    name:", "  smoke-binary:\n    if: ${{ !inputs.dry_run }}\n    name:")
        self.assertTrue(self.check(text))

    def test_rejects_dry_run_written_into_the_command(self) -> None:
        text = mutate(workflow("publish.yml"), 'npm publish "$TARBALL" --provenance --access public --tag next', 'npm publish "$TARBALL" --provenance --access public --tag next --dry-run', 1)
        self.assertTrue(self.check(text))
        text = mutate(workflow("publish.yml"), 'npm publish "$TARBALL" --provenance --access public\n', 'npm publish "$TARBALL" --provenance --access public --dry-run\n', 1)
        self.assertTrue(self.check(text))

    def test_a_publish_step_outside_the_publish_job_is_refused(self) -> None:
        stray = workflow("publish.yml") + "\n  stray:\n    runs-on: ubuntu-26.04\n    steps:\n      - name: Publish\n        env:\n" + self.ENV + "        run: npm publish \"$TARBALL\" --provenance --access public\n"
        self.assertTrue(self.check(stray))


class DryRunSettingTests(unittest.TestCase):
    """npm's dry run is set on the five publish steps only, in any case and under either name, by any route."""

    def check(self, text: str | None = None, name: str = "publish.yml") -> list[str]:
        return CHECKER.dry_run_setting_errors(text if text is not None else workflow(name), name)

    def test_accepts_the_shipped_workflows(self) -> None:
        for name in ("publish.yml", "build-native.yml", "test.yml", "pr-guard.yml"):
            self.assertEqual(self.check(name=name), [], name)

    def test_rejects_the_variable_on_a_job_or_the_workflow(self) -> None:
        for spelling in ("NPM_CONFIG_DRY_RUN", "npm_config_dry_run", "npm_config_dry-run", "Npm_Config_Dry_Run"):
            job = mutate(workflow("publish.yml"), "    name: Publish npm packages\n", f"    name: Publish npm packages\n    env:\n      {spelling}: true\n")
            self.assertTrue(self.check(job), spelling)
            top = mutate(workflow("publish.yml"), "permissions:\n  contents: read\n\njobs:", f"permissions:\n  contents: read\n\nenv:\n  {spelling}: true\n\njobs:", 1)
            self.assertTrue(self.check(top), spelling)

    def test_rejects_the_variable_on_another_step(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        text = mutate(workflow("publish.yml"), step, step + "        env:\n          NPM_CONFIG_DRY_RUN: true\n")
        self.assertTrue(self.check(text))

    def test_rejects_the_variable_in_a_job_of_the_other_release_workflow(self) -> None:
        text = mutate(workflow("build-native.yml"), "    name: Release workflow safety check\n", "    name: Release workflow safety check\n    env:\n      NPM_CONFIG_DRY_RUN: true\n")
        self.assertTrue(self.check(text, "build-native.yml"))

    def test_rejects_a_write_through_github_env_or_npm_config(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        for line in (
            'echo "NPM_CONFIG_DRY_RUN=true" >> "$GITHUB_ENV"',
            "echo npm_config_dry-run=true >> $GITHUB_ENV",
            "npm config set dry-run true",
            "npm set dry-run true",
            "npm config set dry_run=true",
        ):
            text = mutate(workflow("publish.yml"), step, f"      - name: Set it\n        run: {line}\n" + step)
            self.assertTrue(self.check(text), line)

    def test_rejects_npm_config_set_with_options_aliases_or_a_location_in_between(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        for line in (
            "npm config set -g dry-run true",
            "npm config set --location=user dry-run true",
            "npm config --global set dry-run true",
            "npm config --location project set dry_run true",
            "npm c set dry-run true",
            "npm --loglevel=error c set -g dry-run=true",
            "npm set -g dry-run true",
        ):
            text = mutate(workflow("publish.yml"), step, f"      - name: Set it\n        run: {line}\n" + step)
            self.assertTrue(self.check(text), line)

    def test_rejects_the_setting_written_into_an_npmrc(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        for line in (
            'echo "dry-run=true" >> ~/.npmrc',
            "printf 'dry_run=true\\n' >> .npmrc",
            "echo dry-run=true | tee -a .npmrc",
            "|\n          cat >> .npmrc <<EOF\n          dry-run=true\n          EOF",
        ):
            text = mutate(workflow("publish.yml"), step, f"      - name: Set it\n        run: {line}\n" + step)
            self.assertTrue(self.check(text), line)

    def test_accepts_an_npmrc_write_and_a_config_read_that_do_not_set_it(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        for line in ('echo "registry=https://registry.npmjs.org" >> ~/.npmrc', "npm config get dry-run", "npm config set registry https://registry.npmjs.org"):
            text = mutate(workflow("publish.yml"), step, f"      - name: Set it\n        run: {line}\n" + step)
            self.assertEqual(self.check(text), [], line)

    def test_ignores_a_comment_that_names_it(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        text = mutate(workflow("publish.yml"), step, "      # NPM_CONFIG_DRY_RUN and `npm config set dry-run` are not set here.\n" + step)
        self.assertEqual(self.check(text), [])


class TarballTests(unittest.TestCase):
    """npm reads tarballs/x.tgz as the GitHub repo `tarballs/x.tgz`; a publish step names its tarball `./...`, `../...` or `/...`."""

    ASSIGN = 'TARBALL="./tarballs/$(echo "${PACKAGE#@}" | tr / -)-$VERSION.tgz"'

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.tarball_errors(text if text is not None else workflow("publish.yml"), "publish.yml")

    def assigning(self, line: str, count: int = -1) -> str:
        return mutate(workflow("publish.yml"), self.ASSIGN, line, count)

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_accepts_every_quoting_and_prefix_that_starts_with_a_path(self) -> None:
        for line in (
            "TARBALL='./tarballs/x.tgz'",
            "TARBALL=./tarballs/x.tgz",
            'TARBALL="../tarballs/x.tgz"',
            "TARBALL=/work/tarballs/x.tgz",
            'export TARBALL="./tarballs/x.tgz"',
            "export TARBALL=./tarballs/x.tgz",
            "declare -x TARBALL='./tarballs/x.tgz'",
            "declare -r TARBALL=./tarballs/x.tgz",
            "readonly TARBALL=./tarballs/x.tgz",
            "local TARBALL=./tarballs/x.tgz",
            'TARBALL="./tarballs/$(echo x)"',
        ):
            self.assertEqual(self.check(self.assigning(line)), [], line)

    def test_rejects_a_value_that_starts_without_a_path_marker(self) -> None:
        for line in (
            "TARBALL='tarballs/x.tgz'",
            'TARBALL="tarballs/x.tgz"',
            "TARBALL=tarballs/x.tgz",
            "export TARBALL=tarballs/x.tgz",
            "export TARBALL='tarballs/x.tgz'",
            'declare -x TARBALL="tarballs/x.tgz"',
            "readonly TARBALL=tarballs/x.tgz",
            "local TARBALL=tarballs/x.tgz",
            "TARBALL=.tarballs/x.tgz",
            'TARBALL="$PWD/tarballs/x.tgz"',
        ):
            self.assertTrue(self.check(self.assigning(line, 1)), line)

    def test_rejects_a_value_whose_start_is_a_command_substitution(self) -> None:
        for line in ('TARBALL=$(echo "./tarballs/x.tgz")', 'TARBALL="$(echo ./tarballs/x.tgz)"', "TARBALL=`echo ./x.tgz`", "TARBALL="):
            self.assertTrue(self.check(self.assigning(line, 1)), line)

    def test_rejects_a_step_that_assigns_nothing(self) -> None:
        self.assertTrue(self.check(self.assigning("true", 1)))

    def test_rejects_any_other_source_of_the_variable(self) -> None:
        keep = self.ASSIGN
        for extra in ('for TARBALL in x; do :; done', 'read -r TARBALL <<< "x"', 'printf -v TARBALL "%s" x', "read TARBALL"):
            self.assertTrue(self.check(self.assigning(keep + "\n          " + extra, 1)), extra)
        package = '          PACKAGE: "@journeykit/canon-core-linux-arm64"\n'
        self.assertTrue(self.check(mutate(workflow("publish.yml"), package, package + "          TARBALL: ./tarballs/x.tgz\n")))
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        self.assertTrue(self.check(mutate(workflow("publish.yml"), step, '      - name: Set it\n        run: echo "TARBALL=./tarballs/x.tgz" >> "$GITHUB_ENV"\n' + step)))

    def test_ignores_text_that_only_names_the_variable(self) -> None:
        step = "      - name: Upgrade npm for OIDC trusted publishing\n"
        text = mutate(workflow("publish.yml"), step, "      # TARBALL=tarballs/x.tgz would be read as a GitHub repo.\n" + step)
        self.assertEqual(self.check(text), [])

    def test_rejects_a_case_arm_mapfile_append_and_a_quoted_env_key(self) -> None:
        for extra in ("case x in\n            *-*) TARBALL=tarballs/x.tgz ;;\n          esac", "mapfile -t TARBALL < list.txt", "readarray TARBALL < list.txt", "TARBALL+=x"):
            self.assertTrue(self.check(self.assigning(self.ASSIGN + "\n          " + extra, 1)), extra)
        self.assertTrue(self.check(self.assigning("case x in\n            *-*) TARBALL=tarballs/x.tgz ;;\n          esac", 1)))
        package = '          PACKAGE: "@journeykit/canon-core-linux-arm64"\n'
        for key in ('"TARBALL"', "'TARBALL'"):
            self.assertTrue(self.check(mutate(workflow("publish.yml"), package, package + f"          {key}: ./tarballs/x.tgz\n")), key)

    def test_accepts_a_case_arm_that_assigns_a_path(self) -> None:
        self.assertEqual(self.check(self.assigning(self.ASSIGN + "\n          case x in\n            *-*) TARBALL=./tarballs/x.tgz ;;\n          esac", 1)), [])


class ScopeTests(unittest.TestCase):
    """build-native.yml builds by its `scope` job, never by a `paths:` filter."""

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.scope_errors(text if text is not None else workflow("build-native.yml"), "build-native.yml")

    IF = "    if: needs.scope.outputs.skip != 'true'\n"

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_accepts_other_conditions_that_read_the_same(self) -> None:
        for condition in ("${{ needs.scope.outputs.skip != 'true' }}", "!(needs.scope.outputs.skip == 'true')"):
            self.assertEqual(self.check(mutate(workflow("build-native.yml"), self.IF, f"    if: {condition}\n")), [], condition)

    def test_rejects_a_paths_filter_on_pull_request(self) -> None:
        for filter_ in ('    paths:\n      - "crates/**"\n', '    paths-ignore:\n      - "docs/**"\n'):
            self.assertTrue(self.check(mutate(workflow("build-native.yml"), "  pull_request:\n", "  pull_request:\n" + filter_)), filter_)
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "  pull_request:\n", "  pull_request: { paths: ['crates/**'] }\n")))

    def test_a_paths_filter_on_another_trigger_is_not_this_rule(self) -> None:
        text = mutate(workflow("build-native.yml"), "  workflow_dispatch:\n", "  workflow_dispatch:\n  push:\n    paths:\n      - 'crates/**'\n")
        self.assertEqual(self.check(text), [])

    def test_rejects_a_workflow_without_the_scope_job(self) -> None:
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "  scope:\n", "  scoped:\n")))

    def test_rejects_a_scope_job_that_is_slow_or_has_no_output(self) -> None:
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "    name: Decide whether to build\n    runs-on: ubuntu-26.04\n    timeout-minutes: 5\n", "    name: Decide whether to build\n    runs-on: ubuntu-26.04\n    timeout-minutes: 10\n")))
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "      skip: ${{ steps.scope.outputs.skip }}\n", "      other: ${{ steps.scope.outputs.skip }}\n")))

    def test_rejects_a_build_job_that_does_not_wait_for_scope(self) -> None:
        for old, new in (
            ("    needs: [safety, scope, dashboard]\n", "    needs: [safety, dashboard]\n"),
            ("    name: Generate man pages\n    needs: [safety, scope]\n", "    name: Generate man pages\n    needs: safety\n"),
            ("    name: Build dashboard app\n    needs: [safety, scope]\n", "    name: Build dashboard app\n    needs: [safety]\n"),
        ):
            self.assertTrue(self.check(mutate(workflow("build-native.yml"), old, new)), old)

    def test_rejects_a_build_job_without_the_condition(self) -> None:
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), self.IF, "", 1)))

    def test_rejects_an_if_on_the_scope_job_or_its_steps(self) -> None:
        job = "    name: Decide whether to build\n"
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), job, job + "    if: github.event_name == 'pull_request'\n", 1)))
        step = "      - name: Decide from the changed files\n"
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), step, step + "        if: github.event_name == 'pull_request'\n", 1)))

    def test_rejects_a_condition_that_does_not_build_unless_told_to(self) -> None:
        for condition in (
            "needs.scope.outputs.skip == 'false'",
            "needs.scope.outputs.skip",
            "!needs.scope.outputs.skip",
            "needs.scope.outputs.run == 'true'",
            "needs.scope.outputs.skipp != 'true'",
            "needs.scope.result == 'success'",
            "true",
            "needs.scope.outputs.skip != true",
            "false",
        ):
            self.assertTrue(self.check(mutate(workflow("build-native.yml"), self.IF, f"    if: {condition}\n", 1)), condition)


class PreflightTests(unittest.TestCase):
    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.preflight_errors(text if text is not None else workflow("publish.yml"), "publish.yml")

    WHOAMI_GUARD = '          if [ -n "$ABSENT" ]; then\n'

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_whoami_that_always_runs(self) -> None:
        text = mutate(workflow("publish.yml"), self.WHOAMI_GUARD, '          if true; then\n')
        self.assertTrue(self.check(text))

    def test_rejects_whoami_in_the_else_branch(self) -> None:
        text = mutate(workflow("publish.yml"), self.WHOAMI_GUARD, '          if [ -z "$ABSENT" ]; then\n            echo all published\n          else\n')
        text = mutate(text, '          else\n            echo "every package is already on npm', '          fi\n          if [ -n "$ABSENT" ]; then\n            true\n          else\n            echo "every package is already on npm')
        self.assertTrue(self.check(text))

    ELSE_BRANCH = '          else\n            echo "every package is already on npm'

    def with_call(self, call: str) -> str:
        """The shipped workflow with one more line in the branch where no package is absent (no guard there)."""
        return mutate(workflow("publish.yml"), self.ELSE_BRANCH, f"          else\n            {call}\n            echo \"every package is already on npm")

    def test_rejects_an_unguarded_whoami_in_any_form(self) -> None:
        for call in (
            "npm whoami",
            "npm --loglevel=error whoami",
            "npm --registry https://registry.npmjs.org whoami",
            "env FOO=1 npm whoami",
            "FOO=1 npm whoami",
            "command npm whoami",
            "timeout 30 npm whoami",
            "time npm whoami",
            "exec npm whoami",
            "npm \\\n            whoami",
            "npm \\\n              --loglevel=error \\\n              whoami",
            "if ! npm whoami; then exit 1; fi",
            "WHO=$(npm whoami)",
            'echo "$(npm whoami)"',
            "echo `npm whoami`",
            "true && npm whoami",
            "(npm whoami)",
            "/usr/local/bin/npm whoami",
        ):
            self.assertTrue(self.check(self.with_call(call)), call)

    def test_rejects_an_unguarded_whoami_behind_a_redirect_a_parameter_expansion_or_a_heredoc(self) -> None:
        pad = "            "
        for call in (
            "npm whoami>/dev/null",
            "npm whoami</dev/null",
            "npm whoami 2>/dev/null",
            "npm whoami>&2",
            "X=${Y:-$(npm whoami)}",
            "X=${Y:-${Z:-$(npm whoami)}}",
            'X="${Y:-$(npm whoami)}"',
            "X=${#Y}$(npm whoami)",
            f"cat <<EOF\n{pad}can't\n{pad}EOF\n{pad}npm whoami",
            f"cat <<'EOF' >/dev/null\n{pad}it's\n{pad}EOF\n{pad}npm whoami",
            f"cat <<-EOF\n{pad}it's\n{pad}EOF\n{pad}npm whoami",
            "cat <<EOF | npm whoami\n" + pad + "x\n" + pad + "EOF",
        ):
            self.assertTrue(self.check(self.with_call(call)), call)

    def test_accepts_a_heredoc_body_and_an_expansion_that_only_mention_whoami(self) -> None:
        pad = "            "
        for call in (
            f"cat <<EOF\n{pad}npm whoami\n{pad}EOF",
            f"cat <<'EOF'\n{pad}don't npm whoami\n{pad}EOF",
            "echo ${NPM:-npm} whoami",
            "echo ${#Y} npm-whoami",
        ):
            self.assertEqual(self.check(self.with_call(call)), [], call)

    def test_accepts_text_that_only_mentions_whoami(self) -> None:
        for line in ('echo "npm whoami is not run here"', "echo 'npm whoami'", "# npm whoami", "echo \"a $(date) npm whoami\"", "npm view whoami-package"):
            self.assertEqual(self.check(self.with_call(line)), [], line)

    def test_a_guarded_whoami_in_any_form_is_accepted(self) -> None:
        for call in ("npm --loglevel=error whoami", "env FOO=1 npm whoami", "command npm whoami", "timeout 30 npm whoami", "npm --registry https://registry.npmjs.org whoami"):
            text = mutate(workflow("publish.yml"), "            if ! npm whoami; then\n", f"            if ! {call}; then\n")
            self.assertEqual(self.check(text), [], call)

    def test_counts_the_command_substitution_form_as_a_whoami(self) -> None:
        text = mutate(workflow("publish.yml"), "            if ! npm whoami; then\n", "            if ! WHO=$(npm whoami); then\n")
        self.assertEqual(self.check(text), [])
        text = mutate(workflow("publish.yml"), self.WHOAMI_GUARD, '          if true; then\n')
        text = mutate(text, "            if ! npm whoami; then\n", "            if ! WHO=$(npm whoami); then\n")
        self.assertTrue(self.check(text))

    def test_rejects_a_preflight_without_whoami(self) -> None:
        text = mutate(workflow("publish.yml"), "            if ! npm whoami; then\n", "            if ! true; then\n")
        self.assertTrue(self.check(text))

    def test_accepts_the_shipped_whoami_under_the_absence_condition(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_skipped_preflight_in_a_dry_run(self) -> None:
        text = mutate(workflow("publish.yml"), "      - name: Pre-flight npm release state (fresh-release gate + recovery + first-publish token)\n", "      - name: Pre-flight npm release state (fresh-release gate + recovery + first-publish token)\n        if: github.event_name != 'workflow_dispatch' || !inputs.dry_run\n")
        self.assertTrue(self.check(text))

    def test_rejects_a_recovery_with_dry_run_that_does_not_fail_early(self) -> None:
        text = mutate(workflow("publish.yml"), '[ "${DRY_RUN:-false}" = "true" ]', '[ "${DRY_RUN:-false}" = "never" ]')
        self.assertTrue(self.check(text))

    def run_refusal(self, recovery: str, dry_run: str) -> int:
        step = CHECKER._step_block(CHECKER._job_block(workflow("publish.yml"), "read-version"), "Refuse recovery together with dry_run")
        lines = step.splitlines()
        start = next(i for i, line in enumerate(lines) if line.strip() == "run: |") + 1
        indent = len(lines[start]) - len(lines[start].lstrip())
        body = []
        for line in lines[start:]:
            if line.strip() and len(line) - len(line.lstrip()) < indent:
                break
            body.append(line[indent:])
        script = "\n".join(body)
        env = {"PATH": "/usr/bin:/bin", "RECOVERY": recovery, "DRY_RUN": dry_run}
        return subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True, check=False).returncode

    def test_recovery_with_dry_run_fails_and_nothing_else_does(self) -> None:
        self.assertEqual(self.run_refusal("true", "true"), 1)
        self.assertEqual(self.run_refusal("true", "false"), 0)
        self.assertEqual(self.run_refusal("false", "true"), 0)
        self.assertEqual(self.run_refusal("", ""), 0)


class TimeoutTests(unittest.TestCase):
    NAMES = ("build-native.yml", "publish.yml", "test.yml", "pr-guard.yml")

    def test_every_job_of_the_four_workflows_has_a_limit(self) -> None:
        for name in self.NAMES:
            self.assertEqual(CHECKER.timeout_errors(workflow(name), name), [], name)

    def test_rejects_a_job_without_a_limit(self) -> None:
        for name in self.NAMES:
            text = workflow(name)
            job = CHECKER._job_names(text)[0]
            block = CHECKER._job_block(text, job)
            limited = re.sub(r"^    timeout-minutes:.*\n", "", block, flags=re.MULTILINE)
            self.assertNotEqual(limited, block)
            errors = CHECKER.timeout_errors(text.replace(block, limited), name)
            self.assertTrue(errors, (name, errors))

    def test_rejects_a_limit_that_is_not_a_multiple_of_five(self) -> None:
        text = mutate(workflow("publish.yml"), "    timeout-minutes: 15\n", "    timeout-minutes: 12\n", 1)
        self.assertTrue(CHECKER.timeout_errors(text, "publish.yml"))


class NoCompileTests(unittest.TestCase):
    """publish.yml compiles nothing: builds belong to build-native.yml."""

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.no_compile_errors(text if text is not None else workflow("publish.yml"), "publish.yml")

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_compile_step_a_toolchain_install_or_a_dashboard_build(self) -> None:
        for line in (
            "      - run: cargo build --locked --release\n",
            "      - run: rustup default stable\n",
            "      - run: rustc --version\n",
            "      - uses: ./.github/actions/setup-rust\n",
            "      - run: bun run --cwd packages/dashboard build\n",
        ):
            self.assertTrue(self.check(workflow("publish.yml") + line), line)

    def test_rejects_a_build_matrix_row_and_a_build_job(self) -> None:
        self.assertTrue(self.check(workflow("publish.yml") + "          - host: macos-15\n"))
        for job in ("build", "dashboard", "man"):
            self.assertTrue(self.check(workflow("publish.yml") + f"\n  {job}:\n    runs-on: ubuntu-26.04\n    steps: []\n"), job)

    def test_rejects_a_build_setting_in_the_top_level_env(self) -> None:
        for key in ("CARGO_INCREMENTAL: 0", 'MACOSX_DEPLOYMENT_TARGET: "15.0"', "CARGO_TERM_COLOR: always"):
            text = mutate(workflow("publish.yml"), "permissions:\n  contents: read\n\njobs:", f"permissions:\n  contents: read\n\nenv:\n  {key}\n\njobs:", 1)
            self.assertTrue(self.check(text), key)

    def test_a_comment_that_names_them_is_not_a_compile(self) -> None:
        self.assertEqual(self.check(workflow("publish.yml") + "      # cargo build and rustc are not run here\n"), [])


class FetchBuildTests(unittest.TestCase):
    """`fetch-build` finds the run, downloads with its id and a token, verifies, reads every floor, then uploads."""

    RUN_ID = "          run-id: ${{ steps.find.outputs.run_id }}\n"
    REPOSITORY = "          repository: ${{ github.repository }}\n"
    FLOOR = "      - name: Check the binary keeps the floor of its platform\n"

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.fetch_build_errors(text if text is not None else workflow("publish.yml"), "publish.yml")

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_download_without_the_run_the_token_or_the_repository(self) -> None:
        publish = workflow("publish.yml")
        for line in (self.RUN_ID, "          github-token: ${{ github.token }}\n", self.REPOSITORY):
            self.assertTrue(self.check(mutate(publish, line, "", 1)), line)

    def test_rejects_a_digest_mismatch_that_is_not_an_error(self) -> None:
        text = mutate(workflow("publish.yml"), self.REPOSITORY, self.REPOSITORY + "          digest-mismatch: ignore\n", 1)
        self.assertTrue(self.check(text))
        text = mutate(workflow("publish.yml"), self.REPOSITORY, self.REPOSITORY + "          digest-mismatch: error\n", 1)
        self.assertEqual(self.check(text), [])

    def test_rejects_a_missing_floor_step_and_a_missing_target(self) -> None:
        self.assertTrue(self.check(mutate(workflow("publish.yml"), self.FLOOR, "      - name: Something else\n", 1)))
        line = "          python3 scripts/check-release-binary.py --target x86_64-pc-windows-msvc build/canon-binary-x86_64-pc-windows-msvc/canon.exe\n"
        self.assertTrue(self.check(mutate(workflow("publish.yml"), line, "", 1)))

    def test_rejects_a_condition_on_the_floor_step(self) -> None:
        self.assertTrue(self.check(mutate(workflow("publish.yml"), self.FLOOR, self.FLOOR + "        if: always()\n", 1)))

    def test_the_page_size_is_skipped_for_the_aarch64_musl_row_alone(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(mutate(publish, "--target aarch64-unknown-linux-musl --page-size skip ", "--target aarch64-unknown-linux-musl ", 1)))
        self.assertTrue(self.check(mutate(publish, "--target x86_64-unknown-linux-musl build/", "--target x86_64-unknown-linux-musl --page-size skip build/", 1)))

    def test_rejects_a_smoke_floor_check_that_skips_the_page_size_or_is_missing(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(mutate(publish, '--target "${{ matrix.settings.target }}" smoke-bin/canon', '--target "${{ matrix.settings.target }}" --page-size skip smoke-bin/canon')))
        self.assertTrue(self.check(mutate(publish, '          python3 scripts/check-release-binary.py --target "${{ matrix.settings.target }}" smoke-bin/canon\n', "          true\n")))

    def test_rejects_a_job_that_waits_for_build_or_not_for_fetch_build(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(mutate(publish, "    needs: [read-version, fetch-build]\n", "    needs: [read-version, build]\n", 1)))
        self.assertTrue(self.check(mutate(publish, "    needs: [read-version, fetch-build]\n", "    needs: [read-version]\n")))

    def test_rejects_wider_permissions_or_a_missing_read(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(mutate(publish, "      actions: read\n", "")))
        self.assertTrue(self.check(mutate(publish, "      attestations: read\n", "      attestations: write\n")))

    def test_rejects_a_limit_or_runner_that_is_not_the_rule(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(mutate(publish, "    timeout-minutes: 10\n", "    timeout-minutes: 15\n", 1)))
        self.assertTrue(self.check(mutate(publish, "    name: Fetch the binaries of the build-native run\n    needs: read-version\n    runs-on: ubuntu-26.04\n", "    name: Fetch the binaries of the build-native run\n    needs: read-version\n    runs-on: ubuntu-26.04-arm\n")))

    def test_rejects_an_upload_before_the_verification(self) -> None:
        publish = workflow("publish.yml")
        verify = "      - name: Verify the download against the attested manifest\n"
        self.assertTrue(self.check(mutate(publish, verify, "      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2\n        with:\n          name: man-pages\n          path: build/man-pages\n          retention-days: 7\n" + verify, 1)))


class ArtifactContractTests(unittest.TestCase):
    """What build-native.yml uploads and what publish.yml downloads from it are one set of names."""

    def check(self, build: str | None = None, publish: str | None = None) -> list[str]:
        return CHECKER.artifact_contract_errors(build if build is not None else workflow("build-native.yml"), publish if publish is not None else workflow("publish.yml"))

    def test_accepts_the_shipped_workflows(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_an_upload_that_is_not_the_contract(self) -> None:
        self.assertTrue(self.check(build=mutate(workflow("build-native.yml"), "          name: man-pages\n", "          name: man-page\n")))
        self.assertTrue(self.check(build=mutate(workflow("build-native.yml"), "          name: build-manifest-${{ steps.tree.outputs.tree }}\n", "          name: manifest-${{ steps.tree.outputs.tree }}\n")))

    def test_rejects_a_download_the_build_does_not_upload(self) -> None:
        publish = workflow("publish.yml")
        self.assertTrue(self.check(publish=mutate(publish, "          name: man-pages\n          path: build/man-pages\n          run-id", "          name: manpages\n          path: build/man-pages\n          run-id")))
        self.assertTrue(self.check(publish=mutate(publish, "          name: build-manifest-${{ steps.find.outputs.tree }}\n", "          name: manifest-${{ steps.find.outputs.tree }}\n")))

    def test_rejects_a_later_job_that_downloads_what_fetch_build_does_not_upload(self) -> None:
        publish = workflow("publish.yml")
        text = mutate(publish, "          name: canon-binary-x86_64-pc-windows-msvc\n          path: packages/core-win32-x64/bin", "          name: canon-binary-riscv64\n          path: packages/core-win32-x64/bin")
        self.assertTrue(self.check(publish=text))


class RetentionTests(unittest.TestCase):
    def build(self, text: str | None = None) -> list[str]:
        return CHECKER.retention_errors(text if text is not None else workflow("build-native.yml"), "build-native.yml", CHECKER.BUILD_RETENTION)

    def publish(self, text: str | None = None) -> list[str]:
        return CHECKER.retention_errors(text if text is not None else workflow("publish.yml"), "publish.yml", CHECKER.PUBLISH_RETENTION)

    def test_accepts_the_shipped_workflows(self) -> None:
        self.assertEqual(self.build(), [])
        self.assertEqual(self.publish(), [])

    def test_rejects_an_upload_without_a_retention(self) -> None:
        self.assertTrue(self.build(mutate(workflow("build-native.yml"), "          retention-days: 14\n", "", 1)))
        self.assertTrue(self.publish(mutate(workflow("publish.yml"), "          retention-days: 30\n", "")))
        self.assertTrue(self.publish(mutate(workflow("publish.yml"), "          retention-days: 7\n", "", 1)))

    def test_rejects_a_retention_that_is_not_the_rule(self) -> None:
        self.assertTrue(self.build(mutate(workflow("build-native.yml"), "          retention-days: 1\n", "          retention-days: 30\n")))
        self.assertTrue(self.build(mutate(workflow("build-native.yml"), "          retention-days: 14\n", "          retention-days: 90\n", 1)))
        self.assertTrue(self.publish(mutate(workflow("publish.yml"), "          retention-days: 30\n", "          retention-days: 7\n")))


class ManifestJobTests(unittest.TestCase):
    ATTEST_IF = "        if: github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository\n"

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.manifest_job_errors(text if text is not None else workflow("build-native.yml"), "build-native.yml")

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_workflow_without_the_job(self) -> None:
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "  manifest:\n", "  manifested:\n")))

    def test_rejects_a_job_that_does_not_wait_for_the_builds(self) -> None:
        for needs in ("[scope, man]", "[scope, build]"):
            self.assertTrue(self.check(mutate(workflow("build-native.yml"), "    needs: [scope, build, man]\n", f"    needs: {needs}\n")), needs)

    def test_rejects_a_job_without_the_attestation_permissions(self) -> None:
        for scope in ("id-token", "attestations"):
            self.assertTrue(self.check(mutate(workflow("build-native.yml"), f"      {scope}: write\n", "")), scope)

    def test_rejects_the_attestation_permissions_anywhere_else(self) -> None:
        job = "    name: Generate man pages\n"
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), job, job + "    permissions:\n      id-token: write\n")))
        self.assertTrue(self.check(mutate(workflow("build-native.yml"), "permissions:\n  contents: read\n\n# A new push", "permissions:\n  contents: read\n  attestations: write\n\n# A new push")))

    def test_rejects_an_attest_step_that_is_missing_unpinned_or_runs_for_forks(self) -> None:
        build = workflow("build-native.yml")
        self.assertTrue(self.check(mutate(build, self.ATTEST_IF, "")))
        self.assertTrue(self.check(mutate(build, "actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8", "actions/attest-build-provenance@v4")))
        self.assertTrue(self.check(mutate(build, "      - name: Attest the manifest\n", "      - name: Do not attest the manifest\n").replace("actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8", "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9")))

    def test_rejects_a_manifest_that_is_not_recorded_or_not_named_by_the_tree(self) -> None:
        build = workflow("build-native.yml")
        self.assertTrue(self.check(mutate(build, "release-build.py record", "release-build.py noted")))
        self.assertTrue(self.check(mutate(build, "          name: build-manifest-${{ steps.tree.outputs.tree }}\n", "          name: manifest\n")))

    def test_the_job_follows_the_scope_rule(self) -> None:
        build = workflow("build-native.yml")
        self.assertTrue(CHECKER.scope_errors(mutate(build, "    needs: [scope, build, man]\n", "    needs: [build, man]\n"), "build-native.yml"))
        self.assertTrue(CHECKER.scope_errors(with_job_field(build, "manifest", "if", None), "build-native.yml"))


def with_job_field(text: str, job: str, key: str, value: str | None) -> str:
    """`text` with the one-line key of a job (4-space indent) set to `value`, or removed when `value` is None."""
    block = CHECKER._job_block(text, job)
    replacement = "" if value is None else f"    {key}: {value}"
    changed = re.sub(rf"^    {re.escape(key)}:.*(?:\n|$)", replacement + ("\n" if replacement else ""), block, count=1, flags=re.MULTILINE)
    if changed == block:
        raise AssertionError(f"job {job} has no `{key}` to change")
    return text.replace(block, changed, 1)


def swap_steps(text: str, job: str, first: str, second: str) -> str:
    """`text` with the two steps of a job that contain `first` and `second` in each other's place."""
    block = CHECKER._job_block(text, job)
    steps = CHECKER._steps(block)
    a = next(i for i, step in enumerate(steps) if first in step)
    b = next(i for i, step in enumerate(steps) if second in step)
    steps[a], steps[b] = steps[b], steps[a]
    return text.replace(block, "\n".join(steps), 1)


BUILD_SCOPE_SPEC = importlib.util.spec_from_file_location("build_scope", ROOT / "build-scope.py")
assert BUILD_SCOPE_SPEC and BUILD_SCOPE_SPEC.loader
BUILD_SCOPE = importlib.util.module_from_spec(BUILD_SCOPE_SPEC)
BUILD_SCOPE_SPEC.loader.exec_module(BUILD_SCOPE)


class ModeJobTests(unittest.TestCase):
    """`man` and `manifest` run only in a release (or an unnamed mode); `dashboard` and `build` run in every mode."""

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.mode_job_errors(text if text is not None else workflow("build-native.yml"), "build-native.yml")

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_accepts_a_condition_that_reads_the_same(self) -> None:
        condition = "needs.scope.outputs.skip != 'true' && !(needs.scope.outputs.mode == 'quick' || needs.scope.outputs.mode == 'native')"
        self.assertEqual(self.check(with_job_field(workflow("build-native.yml"), "man", "if", condition)), [])

    def test_rejects_man_or_manifest_without_the_mode_condition(self) -> None:
        for job in ("man", "manifest"):
            self.assertTrue(self.check(with_job_field(workflow("build-native.yml"), job, "if", "needs.scope.outputs.skip != 'true'")), job)

    def test_rejects_manifest_or_man_that_runs_in_quick_or_native_only_in_part(self) -> None:
        for job in ("man", "manifest"):
            for condition in (
                "needs.scope.outputs.skip != 'true' && needs.scope.outputs.mode != 'quick'",
                "needs.scope.outputs.skip != 'true' && needs.scope.outputs.mode != 'native'",
            ):
                self.assertTrue(self.check(with_job_field(workflow("build-native.yml"), job, "if", condition)), (job, condition))

    def test_rejects_a_release_condition_that_skips_an_unknown_mode(self) -> None:
        for job in ("man", "manifest"):
            self.assertTrue(self.check(with_job_field(workflow("build-native.yml"), job, "if", "needs.scope.outputs.skip != 'true' && needs.scope.outputs.mode == 'release'")), job)

    def test_rejects_a_mode_condition_on_the_dashboard_or_the_build(self) -> None:
        for job in ("dashboard", "build"):
            self.assertTrue(self.check(with_job_field(workflow("build-native.yml"), job, "if", "needs.scope.outputs.skip != 'true' && needs.scope.outputs.mode != 'quick'")), job)


class MatrixModeTests(unittest.TestCase):
    """The rows `build` runs by mode: one for `quick`, four for the others."""

    NAME = "build-native.yml"

    def check(self, text: str | None = None, exclude_for=BUILD_SCOPE.exclude_for) -> list[str]:
        return CHECKER.matrix_mode_errors(text if text is not None else workflow(self.NAME), self.NAME, exclude_for)

    def test_accepts_the_shipped_workflow_and_script(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_quick_run_that_builds_four_rows(self) -> None:
        self.assertTrue(self.check(exclude_for=lambda mode: []))

    def test_rejects_a_release_or_native_or_unnamed_mode_that_leaves_a_row_out(self) -> None:
        for left_out in ("release", "native", "no-such-mode", ""):
            self.assertTrue(self.check(exclude_for=lambda mode, left_out=left_out: BUILD_SCOPE.QUICK_EXCLUDE if mode in ("quick", left_out) else []), left_out)

    def test_rejects_a_matrix_that_does_not_read_the_exclude_the_scope_writes(self) -> None:
        text = re.sub(r"^        exclude:.*\n", "", workflow(self.NAME), flags=re.MULTILINE)
        self.assertTrue(self.check(text))
        self.assertTrue(self.check(mutate(workflow(self.NAME), "fromJSON(needs.scope.outputs.exclude", "fromJSON(needs.scope.outputs.other")))

    def test_rejects_a_row_the_quick_run_would_also_build(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), '            quick: "false"\n', "", 1)))


class CacheSaveTests(unittest.TestCase):
    """`build` and `man` restore in every run and save in a `release/` pull request run or a manual run, after cleaning."""

    NAME = "build-native.yml"
    RELEASE_OR_MANUAL = "(startsWith(github.head_ref, 'release/') || github.event_name == 'workflow_dispatch')"

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.cache_save_errors(text if text is not None else workflow(self.NAME), self.NAME)

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_save_that_runs_in_every_pull_request(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), self.RELEASE_OR_MANUAL, "true")))
        self.assertTrue(self.check(mutate(workflow(self.NAME), "        if: ${{ steps.cache.outputs.cache-hit != 'true' && steps.strip.outcome == 'success' && " + self.RELEASE_OR_MANUAL + " }}\n", "")))

    def test_rejects_a_save_that_leaves_out_the_manual_run(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), self.RELEASE_OR_MANUAL, "startsWith(github.head_ref, 'release/')")))

    def test_rejects_a_save_that_leaves_out_the_release_pull_request(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), self.RELEASE_OR_MANUAL, "github.event_name == 'workflow_dispatch'")))

    def test_rejects_a_save_that_does_not_need_the_clean_step_to_succeed(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "steps.strip.outcome == 'success' && ", "")))

    def test_rejects_a_save_that_saves_over_an_exact_restore(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "steps.cache.outputs.cache-hit != 'true' && steps.strip.outcome", "steps.strip.outcome")))

    def test_rejects_a_save_before_the_clean_step(self) -> None:
        for job in ("build", "man"):
            self.assertTrue(self.check(swap_steps(workflow(self.NAME), job, "actions/cache/save@", "ci-strip-workspace-build.sh")), job)

    def test_rejects_a_clean_or_save_that_can_fail_the_job(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "        continue-on-error: true\n", "", 1)))
        self.assertTrue(self.check(mutate(workflow(self.NAME), "        continue-on-error: true\n", "")))

    def test_rejects_a_clean_step_the_save_cannot_read(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "        id: strip\n", "        id: clean\n")))

    def test_rejects_a_restore_that_does_not_run_in_every_run(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "        id: cache\n", "        id: cache\n        if: github.event_name != 'pull_request'\n")))
        self.assertTrue(self.check(mutate(workflow(self.NAME), "actions/cache/restore@", "actions/cache@")))


class TestedTests(unittest.TestCase):
    """test.yml: `tested` reads the artifacts, a seeding run is a push to main of a tree it says `true` of, `record` is the one uploader."""

    NAME = "test.yml"
    SEED = "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && needs.tested.outputs.tested == 'true' }}"

    def check(self, text: str | None = None) -> list[str]:
        return CHECKER.tested_errors(text if text is not None else workflow(self.NAME), self.NAME)

    def test_accepts_the_shipped_workflow(self) -> None:
        self.assertEqual(self.check(), [])

    def test_rejects_a_tested_job_that_cannot_read_the_artifacts_or_can_write(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "      actions: read\n", "")))
        self.assertTrue(self.check(mutate(workflow(self.NAME), "      actions: read\n", "      actions: write\n")))

    def test_rejects_a_job_that_does_not_wait_for_tested(self) -> None:
        for job in ("offline", "launcher", "live-tier"):
            self.assertTrue(self.check(with_job_field(workflow(self.NAME), job, "needs", "[]")), job)

    def test_rejects_a_job_that_runs_in_a_seeding_run_though_it_must_not(self) -> None:
        for job in ("launcher", "platformer"):
            self.assertTrue(self.check(with_job_field(workflow(self.NAME), job, "if", "${{ !cancelled() }}")), job)

    def test_rejects_a_job_that_does_not_run_when_the_answer_is_not_true(self) -> None:
        for job in ("launcher", "platformer"):
            self.assertTrue(self.check(with_job_field(workflow(self.NAME), job, "if", "${{ !cancelled() && needs.tested.outputs.tested == 'false' }}")), job)
        self.assertTrue(self.check(with_job_field(workflow(self.NAME), "offline", "if", "${{ needs.tested.result == 'success' }}")))

    def test_rejects_a_seeding_run_that_is_not_a_push_to_main_of_a_tested_tree(self) -> None:
        build = workflow(self.NAME)
        for seed in (
            "${{ needs.tested.outputs.tested == 'true' }}",
            "${{ github.event_name == 'push' && needs.tested.outputs.tested == 'true' }}",
            "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && needs.tested.outputs.tested != 'false' }}",
            "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
        ):
            self.assertTrue(self.check(mutate(build, "      SEED: " + self.SEED + "\n", "      SEED: " + seed + "\n", 1)), seed)

    def test_rejects_a_record_that_does_not_wait_for_every_job_or_runs_for_more_than_a_pull_request(self) -> None:
        self.assertTrue(self.check(with_job_field(workflow(self.NAME), "record", "needs", "[offline, clippy]")))
        for condition in ("${{ github.event_name != 'workflow_dispatch' }}", "${{ always() }}", "true"):
            self.assertTrue(self.check(with_job_field(workflow(self.NAME), "record", "if", condition)), condition)

    def test_rejects_a_record_that_does_not_replace_the_marker_and_another_job_that_uploads_one(self) -> None:
        self.assertTrue(self.check(mutate(workflow(self.NAME), "          overwrite: true\n", "")))
        step = "      - name: Install workspace dependencies from the frozen lockfile\n"
        text = mutate(workflow(self.NAME), step, "      - uses: actions/upload-artifact@x\n        with:\n          name: tested-x\n" + step, 1)
        self.assertTrue(self.check(text))


if __name__ == "__main__":
    unittest.main()
