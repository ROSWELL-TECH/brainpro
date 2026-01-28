"""Test 04: Build operations (cargo build, cargo test)."""

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_not_contains,
    assert_output_matches,
    assert_tool_called,
)


class TestBuild:
    """Build operation tests."""

    def test_cargo_build(self, runner: BrainproRunner):
        """Agent can run cargo build."""
        prompt = "Build the Rust project in fixtures/hello_repo using cargo build --release"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_not_contains("error[E", result.output)  # No compiler errors
        assert_tool_called("Bash", result.output)

    def test_cargo_test(self, runner: BrainproRunner):
        """Agent can run cargo test."""
        prompt = "Run the tests for the project in fixtures/hello_repo and tell me if they pass"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_output_matches("(pass|PASSED|ok|OK)", result.output)
        assert_tool_called("Bash", result.output)
