#!/usr/bin/env python3
"""Offline fixture tests for the canonical knowledge checker."""

from __future__ import annotations

import datetime as dt
import importlib.util
import pathlib
import unittest
import unittest.mock as mock


ROOT = pathlib.Path(__file__).resolve().parent
FIXTURES = ROOT / "test-fixtures" / "knowledge"
SPEC = importlib.util.spec_from_file_location("check_knowledge", ROOT / "check-knowledge.py")
assert SPEC and SPEC.loader
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class KnowledgeFixtureTests(unittest.TestCase):
    def errors(self, name: str, *, as_of: str = "2026-10-01") -> list[str]:
        root = FIXTURES / name
        return CHECKER.check_index(root, root / "canon/knowledge-index.json", as_of=dt.date.fromisoformat(as_of))

    def test_accepts_source_projection_and_empirical_memory_entries(self) -> None:
        self.assertEqual(self.errors("accept"), [])

    def test_rejects_broken_primary_link(self) -> None:
        errors = self.errors("broken-link")
        self.assertTrue(any("broken primary link" in error for error in errors), errors)

    def test_rejects_stale_active_document_at_explicit_as_of(self) -> None:
        errors = self.errors("stale")
        self.assertTrue(any("stale review_date" in error for error in errors), errors)
    def test_live_default_does_not_borrow_index_review_date(self) -> None:
        root = FIXTURES / "accept"
        class FutureDateTime(dt.datetime):
            @classmethod
            def now(cls, tz=None):
                return cls(2027, 10, 1, tzinfo=tz)
        with mock.patch.object(CHECKER.dt, "datetime", FutureDateTime):
            errors = CHECKER.check_index(root, root / "canon/knowledge-index.json")
        self.assertTrue(any("stale review_date" in error and "as_of 2027-10-01" in error for error in errors), errors)

    def test_rejects_duplicate_path(self) -> None:
        errors = self.errors("duplicate")
        self.assertTrue(any("duplicate path" in error for error in errors), errors)

    def test_rejects_path_traversal(self) -> None:
        errors = self.errors("traversal")
        self.assertTrue(any("traversal" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
