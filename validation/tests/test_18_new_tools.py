"""Test 18: New tools (TodoWrite, plan mode tools, session resume)."""

import pytest

from harness.runner import BrainproRunner
from harness.fixtures import SessionsDir
from harness.assertions import (
    assert_exit_code,
    assert_output_matches,
    assert_tool_called,
)


class TestNewTools:
    """New tools tests."""

    def test_todowrite_basic(self, runner: BrainproRunner):
        """TodoWrite tool creates task list."""
        prompt = (
            "Create a todo list with exactly 3 tasks for adding a greeting function "
            "to fixtures/hello_repo. Use the TodoWrite tool."
        )

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        assert_tool_called("TodoWrite", result.output)
        # Should show task display box
        assert_output_matches("(Tasks|pending|in_progress|completed)", result.output)

    def test_plan_mode_tools(self, runner: BrainproRunner):
        """EnterPlanMode and ExitPlanMode tools work."""
        prompt = (
            "Use the EnterPlanMode tool to enter planning mode, then describe the "
            "structure of fixtures/hello_repo, then use ExitPlanMode to exit."
        )

        result = runner.oneshot(prompt)

        assert_exit_code(0, result.exit_code)
        # Should call either tool or mention plan mode
        assert_output_matches(
            "(EnterPlanMode|ExitPlanMode|plan.?mode|planning)", result.output
        )

    @pytest.mark.gateway_only
    def test_session_resume(self, runner: BrainproRunner, sessions_dir: SessionsDir):
        """Session persistence and resume."""
        # Run a simple command then exit
        result = runner.repl(
            "What is 2+2? Just say the number.",
            "/exit",
        )

        assert_exit_code(0, result.exit_code)

        # Check that a session was saved
        sessions = sessions_dir.list_sessions()
        assert len(sessions) > 0, "No session files found"

        # Get the session ID
        session_id = sessions_dir.get_latest_session_id()
        assert session_id is not None

        # Try to resume the session
        resume_result = runner.oneshot(
            "What did I just ask you?",
            "--resume",
            session_id,
        )

        # Resume should work without error
        assert_exit_code(0, resume_result.exit_code)
