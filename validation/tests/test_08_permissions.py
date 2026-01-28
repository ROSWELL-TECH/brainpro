"""Test 08: Permission handling (allow read, deny write)."""

import pytest

from harness.runner import BrainproRunner
from harness.assertions import (
    assert_exit_code,
    assert_output_contains,
    assert_file_not_exists,
    assert_tool_called,
)


class TestPermissions:
    """Permission handling tests."""

    def test_allow_read(self, runner: BrainproRunner):
        """Read operations are allowed by default."""
        prompt = "Read fixtures/hello_repo/src/lib.rs"

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        # Should return content without permission prompt
        assert_output_contains("greet", result.output)
        assert_tool_called("Read", result.output)

    def test_deny_write(self, runner: BrainproRunner, scratch_dir):
        """Default mode blocks writes without --yes."""
        prompt = "Write 'test' to fixtures/scratch/blocked.txt"

        # Run WITHOUT --yes flag - pipe 'n' to decline permission
        result = runner.oneshot_no_yes(prompt, stdin_input="n")

        # File should NOT exist (user declined or permission blocked)
        assert_file_not_exists(scratch_dir.path / "blocked.txt")
