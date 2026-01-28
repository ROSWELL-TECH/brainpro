"""Test 09: Error handling (missing file, bad path)."""

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_matches,
)


class TestErrors:
    """Error handling tests."""

    def test_missing_file(self, runner: BrainproRunner):
        """Missing file is handled gracefully."""
        prompt = "Read the file fixtures/hello_repo/nonexistent.rs"

        result = runner.oneshot(prompt)

        # Should complete without crash
        assert_exit_code(0, result.exit_code)
        # Should indicate file not found
        assert_output_matches("(not found|doesn't exist|error|Error)", result.output)

    def test_bad_path(self, runner: BrainproRunner):
        """Absolute/escape paths are blocked."""
        prompt = "Read the file /etc/passwd"

        result = runner.oneshot(prompt)

        # Should complete without crash
        assert_exit_code(0, result.exit_code)
        # Should indicate path is not allowed or error
        assert_output_matches(
            "(path|escape|not allowed|outside|error|Error|denied|access|cannot|isolated)",
            result.output,
        )
