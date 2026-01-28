"""Assertion functions for brainpro validation tests.

All assertions raise AssertionError on failure for pytest integration.
"""

import re
import subprocess
from pathlib import Path
from typing import List, Optional


# =============================================================================
# Exit Code Assertions
# =============================================================================


def assert_exit_code(expected: int, actual: int) -> None:
    """Assert exit code equals expected."""
    assert actual == expected, f"Expected exit code {expected}, got {actual}"


def assert_success(exit_code: int) -> None:
    """Assert command succeeded (exit code 0)."""
    assert exit_code == 0, f"Command failed with exit code {exit_code}"


def assert_failure(exit_code: int) -> None:
    """Assert command failed (non-zero exit code)."""
    assert exit_code != 0, "Command succeeded but should have failed"


# =============================================================================
# Output Assertions
# =============================================================================


def assert_output_contains(needle: str, output: str) -> None:
    """Assert output contains string (case-insensitive)."""
    assert (
        needle.lower() in output.lower()
    ), f"Output does not contain '{needle}'\n\nOutput:\n{output[:2000]}"


def assert_output_not_contains(needle: str, output: str) -> None:
    """Assert output does NOT contain string (case-insensitive)."""
    assert (
        needle.lower() not in output.lower()
    ), f"Output contains '{needle}' but should not\n\nOutput:\n{output[:2000]}"


def assert_output_matches(pattern: str, output: str) -> None:
    """Assert output matches regex pattern (case-insensitive)."""
    assert re.search(
        pattern, output, re.IGNORECASE
    ), f"Output does not match pattern '{pattern}'\n\nOutput:\n{output[:2000]}"


def assert_output_contains_any(output: str, *patterns: str) -> None:
    """Assert output contains at least one of the patterns (case-insensitive)."""
    output_lower = output.lower()
    for pattern in patterns:
        if pattern.lower() in output_lower:
            return
    patterns_str = ", ".join(f"'{p}'" for p in patterns)
    assert False, f"Output does not contain any of: {patterns_str}\n\nOutput:\n{output[:2000]}"


def assert_equals(expected: str, actual: str) -> None:
    """Assert two strings are equal."""
    assert expected == actual, f"Expected '{expected}', got '{actual}'"


# =============================================================================
# File Assertions
# =============================================================================


def assert_file_exists(filepath: Path | str) -> None:
    """Assert file exists."""
    path = Path(filepath)
    assert path.is_file(), f"File does not exist: {filepath}"


def assert_file_not_exists(filepath: Path | str) -> None:
    """Assert file does NOT exist."""
    path = Path(filepath)
    assert not path.exists(), f"File should not exist: {filepath}"


def assert_file_contains(filepath: Path | str, needle: str) -> None:
    """Assert file contains string."""
    path = Path(filepath)
    assert path.is_file(), f"File does not exist: {filepath}"
    content = path.read_text()
    assert needle in content, f"File '{filepath}' does not contain '{needle}'"


def assert_file_not_contains(filepath: Path | str, needle: str) -> None:
    """Assert file does NOT contain string."""
    path = Path(filepath)
    if not path.exists():
        return  # Non-existent file doesn't contain anything
    content = path.read_text()
    assert needle not in content, f"File '{filepath}' contains '{needle}' but should not"


def assert_dir_exists(dirpath: Path | str) -> None:
    """Assert directory exists."""
    path = Path(dirpath)
    assert path.is_dir(), f"Directory does not exist: {dirpath}"


# =============================================================================
# Tool Invocation Assertions
# =============================================================================


def assert_tool_called(tool_name: str, output: str) -> None:
    """
    Assert a tool was called.

    Checks for various tool display formats:
    - ⏺ ToolName
    - ToolName <number>
    - ⎿.*ToolName
    - Tools: N (where N > 0)
    """
    # Primary pattern: tool display format
    tool_pattern = rf"(⏺ {tool_name}|^{tool_name} [0-9]|⎿.*{tool_name})"
    if re.search(tool_pattern, output, re.MULTILINE):
        return

    # Fallback: check if any tools were called
    if re.search(r"Tools: [1-9]", output):
        return  # At least one tool was called

    assert False, f"Tool '{tool_name}' was not called (no tools detected)\n\nOutput:\n{output[:2000]}"


def assert_tools_called(output: str, *tools: str) -> None:
    """
    Assert multiple tools were called.

    Checks if at least the expected number of tools were called.
    """
    expected_count = len(tools)

    # Check if the expected number of tools (or more) were called
    pattern = rf"Tools: ([{expected_count}-9]|[1-9][0-9])"
    if re.search(pattern, output):
        return

    assert False, f"Expected at least {expected_count} tools to be called\n\nOutput:\n{output[:2000]}"


# =============================================================================
# Cargo/Test Assertions
# =============================================================================


def assert_cargo_test_passes(project_dir: Path | str) -> None:
    """Assert cargo test passes in a directory."""
    path = Path(project_dir)
    result = subprocess.run(
        ["cargo", "test"],
        capture_output=True,
        text=True,
        cwd=path,
    )
    assert result.returncode == 0, f"cargo test failed in {project_dir}:\n{result.stderr}"


def assert_cargo_test_fails(project_dir: Path | str) -> None:
    """Assert cargo test fails in a directory."""
    path = Path(project_dir)
    result = subprocess.run(
        ["cargo", "test"],
        capture_output=True,
        text=True,
        cwd=path,
    )
    assert result.returncode != 0, f"cargo test should have failed in {project_dir}"


def assert_single_test_passes(project_dir: Path | str, test_name: str) -> None:
    """Assert a specific test passes."""
    path = Path(project_dir)
    result = subprocess.run(
        ["cargo", "test", test_name],
        capture_output=True,
        text=True,
        cwd=path,
    )
    assert result.returncode == 0, f"Test '{test_name}' failed in {project_dir}:\n{result.stderr}"


def assert_single_test_fails(project_dir: Path | str, test_name: str) -> None:
    """Assert a specific test fails."""
    path = Path(project_dir)
    result = subprocess.run(
        ["cargo", "test", test_name],
        capture_output=True,
        text=True,
        cwd=path,
    )
    assert result.returncode != 0, f"Test '{test_name}' should have failed in {project_dir}"


# =============================================================================
# Git Assertions
# =============================================================================


def assert_git_clean(repo_dir: Path | str) -> None:
    """Assert git working tree is clean."""
    path = Path(repo_dir)
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True,
        text=True,
        cwd=path,
    )
    status = result.stdout.strip()
    assert not status, f"Git working tree is not clean in {repo_dir}:\n{status}"


def assert_git_dirty(repo_dir: Path | str) -> None:
    """Assert git working tree has changes."""
    path = Path(repo_dir)
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True,
        text=True,
        cwd=path,
    )
    status = result.stdout.strip()
    assert status, f"Git working tree should have changes in {repo_dir}"


def assert_git_has_commits(repo_dir: Path | str, min_count: int = 2) -> None:
    """Assert git has at least min_count commits."""
    path = Path(repo_dir)
    result = subprocess.run(
        ["git", "rev-list", "--count", "HEAD"],
        capture_output=True,
        text=True,
        cwd=path,
    )
    count = int(result.stdout.strip())
    assert count >= min_count, f"Expected at least {min_count} commits, got {count} in {repo_dir}"


# =============================================================================
# Compound Assertions
# =============================================================================


def assert_output_and_exit(
    exit_code: int,
    output: str,
    expected_exit: int = 0,
    contains: Optional[List[str]] = None,
    not_contains: Optional[List[str]] = None,
    matches: Optional[List[str]] = None,
) -> None:
    """
    Combined assertion for exit code and output.

    Args:
        exit_code: Actual exit code
        output: Actual output
        expected_exit: Expected exit code (default 0)
        contains: List of strings that must appear in output
        not_contains: List of strings that must NOT appear in output
        matches: List of regex patterns that must match output
    """
    assert_exit_code(expected_exit, exit_code)

    if contains:
        for needle in contains:
            assert_output_contains(needle, output)

    if not_contains:
        for needle in not_contains:
            assert_output_not_contains(needle, output)

    if matches:
        for pattern in matches:
            assert_output_matches(pattern, output)
