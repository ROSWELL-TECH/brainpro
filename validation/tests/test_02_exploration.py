"""Test 02: Codebase exploration (describe, structure, find tests)."""

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
)


class TestExploration:
    """Codebase exploration tests."""

    def test_describe_codebase(self, runner: BrainproRunner):
        """Agent can describe a codebase."""
        prompt = "Describe the project in fixtures/hello_repo. What kind of project is it?"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("Rust", result.output)

    def test_understand_structure(self, runner: BrainproRunner):
        """Agent can understand file structure."""
        prompt = "What functions are defined in fixtures/hello_repo/src/lib.rs?"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("greet", result.output)

    def test_find_tests(self, runner: BrainproRunner):
        """Agent can find tests in codebase."""
        prompt = "Find the tests defined in fixtures/hello_repo and list their names"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_contains("test_greet", result.output)
