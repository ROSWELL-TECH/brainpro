"""Fixture management for brainpro tests."""

import shutil
import subprocess
from pathlib import Path
from typing import Optional

from .modes import ModeConfig


class ScratchDir:
    """Manages the scratch directory for testing."""

    def __init__(self, config: ModeConfig):
        self.config = config
        self.path = config.scratch_dir

    def reset(self) -> None:
        """Reset the scratch directory to empty state."""
        # Try to clean using docker first (handles permission issues from containers)
        try:
            subprocess.run(
                [
                    "docker",
                    "run",
                    "--rm",
                    "-v",
                    f"{self.path}:/scratch",
                    "alpine",
                    "sh",
                    "-c",
                    "rm -rf /scratch/* /scratch/.[!.]* 2>/dev/null; chown -R 1000:1000 /scratch",
                ],
                capture_output=True,
                timeout=30,
            )
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass  # Docker not available or timed out, use normal cleanup

        # Normal cleanup
        if self.path.exists():
            shutil.rmtree(self.path, ignore_errors=True)
        self.path.mkdir(parents=True, exist_ok=True)

    def cleanup(self) -> None:
        """Clean up scratch directory."""
        if self.path.exists():
            shutil.rmtree(self.path, ignore_errors=True)


class MockWebapp:
    """Manages the mock_webapp scratch copy for testing."""

    def __init__(self, config: ModeConfig):
        self.config = config
        self.source = config.mock_webapp_dir
        self.scratch = config.mock_webapp_scratch

    def reset(self) -> None:
        """
        Reset mock_webapp_scratch to a fresh copy.

        Creates a git repo in the scratch copy for testing.
        """
        # Remove existing scratch
        if self.scratch.exists():
            shutil.rmtree(self.scratch, ignore_errors=True)

        # Copy mock_webapp to scratch
        shutil.copytree(self.source, self.scratch)

        # Initialize git repo
        subprocess.run(
            ["git", "init", "-q"],
            cwd=self.scratch,
            capture_output=True,
        )
        subprocess.run(
            ["git", "add", "."],
            cwd=self.scratch,
            capture_output=True,
        )
        subprocess.run(
            ["git", "commit", "-q", "-m", "Initial commit"],
            cwd=self.scratch,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.email", "test@example.com"],
            cwd=self.scratch,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test User"],
            cwd=self.scratch,
            capture_output=True,
        )

    def cleanup(self) -> None:
        """Clean up mock_webapp scratch directory."""
        if self.scratch.exists():
            shutil.rmtree(self.scratch, ignore_errors=True)

    @property
    def path(self) -> Path:
        """Return the scratch directory path."""
        return self.scratch


class SessionsDir:
    """Manages the sessions directory for testing."""

    def __init__(self):
        self.path = Path.home() / ".brainpro" / "sessions"

    def reset(self) -> None:
        """Clear all sessions."""
        if self.path.exists():
            shutil.rmtree(self.path, ignore_errors=True)

    def list_sessions(self) -> list[Path]:
        """List all session files."""
        if not self.path.exists():
            return []
        return list(self.path.glob("*.json"))

    def get_latest_session_id(self) -> Optional[str]:
        """Get the ID of the most recent session."""
        sessions = self.list_sessions()
        if not sessions:
            return None
        latest = max(sessions, key=lambda p: p.stat().st_mtime)
        return latest.stem
