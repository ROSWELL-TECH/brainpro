"""Brainpro runner for executing yo binary."""

import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional

from .modes import ModeConfig


@dataclass
class RunResult:
    """Result of running brainpro."""

    exit_code: int
    output: str  # combined stdout + stderr
    stdout: str
    stderr: str


class BrainproRunner:
    """Runs brainpro yo binary in various modes."""

    def __init__(self, config: ModeConfig, working_dir: Optional[Path] = None):
        """
        Initialize runner.

        Args:
            config: Mode configuration
            working_dir: Working directory for commands (defaults to project root)
        """
        self.config = config
        self.working_dir = working_dir or config.project_root

    def _build_command(
        self,
        prompt: Optional[str] = None,
        yes: bool = True,
        extra_args: Optional[List[str]] = None,
    ) -> List[str]:
        """Build command line for yo binary."""
        cmd = [str(self.config.binary_path)]

        # Add gateway URL if in gateway mode
        if self.config.gateway_url:
            cmd.extend(["--gateway", self.config.gateway_url])

        # Add prompt if provided (oneshot mode)
        if prompt:
            cmd.extend(["-p", prompt])

        # Add --yes for auto-approval
        if yes:
            cmd.append("--yes")

        # Add any extra arguments
        if extra_args:
            cmd.extend(extra_args)

        return cmd

    def oneshot(
        self,
        prompt: str,
        *extra_args: str,
        yes: bool = True,
        timeout: int = 120,
        working_dir: Optional[Path] = None,
    ) -> RunResult:
        """
        Run brainpro in one-shot mode with a single prompt.

        Args:
            prompt: The prompt to send
            *extra_args: Additional command line arguments
            yes: Whether to auto-approve (--yes flag)
            timeout: Timeout in seconds
            working_dir: Override working directory

        Returns:
            RunResult with exit code and output
        """
        cmd = self._build_command(prompt=prompt, yes=yes, extra_args=list(extra_args))
        cwd = working_dir or self.working_dir

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=cwd,
            )
            return RunResult(
                exit_code=result.returncode,
                output=result.stdout + result.stderr,
                stdout=result.stdout,
                stderr=result.stderr,
            )
        except subprocess.TimeoutExpired as e:
            return RunResult(
                exit_code=-1,
                output=f"Timeout after {timeout}s: {e}",
                stdout=e.stdout.decode() if e.stdout else "",
                stderr=e.stderr.decode() if e.stderr else "",
            )

    def repl(
        self,
        *commands: str,
        yes: bool = True,
        timeout: int = 180,
        working_dir: Optional[Path] = None,
    ) -> RunResult:
        """
        Run brainpro in REPL mode with piped commands.

        Args:
            *commands: Commands to send (each as a separate line)
            yes: Whether to auto-approve (--yes flag)
            timeout: Timeout in seconds
            working_dir: Override working directory

        Returns:
            RunResult with exit code and output
        """
        cmd = self._build_command(prompt=None, yes=yes)
        cwd = working_dir or self.working_dir

        # Join commands with newlines
        input_text = "\n".join(commands) + "\n"

        try:
            result = subprocess.run(
                cmd,
                input=input_text,
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=cwd,
            )
            return RunResult(
                exit_code=result.returncode,
                output=result.stdout + result.stderr,
                stdout=result.stdout,
                stderr=result.stderr,
            )
        except subprocess.TimeoutExpired as e:
            return RunResult(
                exit_code=-1,
                output=f"Timeout after {timeout}s: {e}",
                stdout=e.stdout.decode() if e.stdout else "",
                stderr=e.stderr.decode() if e.stderr else "",
            )

    def oneshot_no_yes(
        self,
        prompt: str,
        stdin_input: str = "n",
        timeout: int = 120,
        working_dir: Optional[Path] = None,
    ) -> RunResult:
        """
        Run brainpro in one-shot mode WITHOUT --yes flag.

        Useful for testing permission denial scenarios.

        Args:
            prompt: The prompt to send
            stdin_input: Input to pipe to stdin (e.g., 'n' to decline)
            timeout: Timeout in seconds
            working_dir: Override working directory

        Returns:
            RunResult with exit code and output
        """
        cmd = self._build_command(prompt=prompt, yes=False)
        cwd = working_dir or self.working_dir

        try:
            result = subprocess.run(
                cmd,
                input=stdin_input + "\n",
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=cwd,
            )
            return RunResult(
                exit_code=result.returncode,
                output=result.stdout + result.stderr,
                stdout=result.stdout,
                stderr=result.stderr,
            )
        except subprocess.TimeoutExpired as e:
            return RunResult(
                exit_code=-1,
                output=f"Timeout after {timeout}s: {e}",
                stdout=e.stdout.decode() if e.stdout else "",
                stderr=e.stderr.decode() if e.stderr else "",
            )

    def with_working_dir(self, working_dir: Path) -> "BrainproRunner":
        """Return a new runner with a different working directory."""
        return BrainproRunner(self.config, working_dir)
