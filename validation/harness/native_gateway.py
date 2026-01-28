"""Native gateway process management."""

import os
import signal
import subprocess
import time
from pathlib import Path
from typing import Optional
import urllib.request
import urllib.error


class NativeGateway:
    """
    Manages native brainpro-gateway and brainpro-agent processes.

    Starts the gateway and agent as subprocesses, waits for health,
    and cleans up on stop.
    """

    GATEWAY_PORT = 18789
    SOCKET_PATH = "/run/brainpro.sock"
    HEALTH_URL = "http://localhost:18789/health"
    WS_URL = "ws://localhost:18789/ws"

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.gateway_bin = project_root / "target" / "release" / "brainpro-gateway"
        self.agent_bin = project_root / "target" / "release" / "brainpro-agent"
        self.gateway_proc: Optional[subprocess.Popen] = None
        self.agent_proc: Optional[subprocess.Popen] = None

    def start(self, timeout: int = 60) -> str:
        """
        Start the gateway and agent processes.

        Args:
            timeout: Max seconds to wait for health check

        Returns:
            WebSocket URL for the gateway

        Raises:
            RuntimeError: If processes fail to start or health check times out
        """
        # Verify binaries exist
        if not self.gateway_bin.exists():
            raise RuntimeError(f"Gateway binary not found: {self.gateway_bin}")
        if not self.agent_bin.exists():
            raise RuntimeError(f"Agent binary not found: {self.agent_bin}")

        # Clean up any existing socket
        socket_path = Path(self.SOCKET_PATH)
        if socket_path.exists():
            socket_path.unlink()

        # Start agent first (listens on unix socket)
        self.agent_proc = subprocess.Popen(
            [str(self.agent_bin)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=self.project_root,
        )

        # Give agent a moment to start
        time.sleep(0.5)
        if self.agent_proc.poll() is not None:
            _, stderr = self.agent_proc.communicate()
            raise RuntimeError(f"Agent failed to start: {stderr.decode()}")

        # Start gateway (connects to agent via unix socket)
        self.gateway_proc = subprocess.Popen(
            [str(self.gateway_bin)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=self.project_root,
        )

        # Wait for health endpoint
        if not self._wait_for_health(timeout):
            self.stop()
            raise RuntimeError("Gateway failed to become healthy")

        return self.WS_URL

    def _wait_for_health(self, timeout: int) -> bool:
        """Wait for the health endpoint to respond."""
        start = time.time()
        while time.time() - start < timeout:
            try:
                with urllib.request.urlopen(self.HEALTH_URL, timeout=2) as response:
                    if response.status == 200:
                        return True
            except (urllib.error.URLError, OSError):
                pass
            time.sleep(1)
        return False

    def stop(self) -> None:
        """Stop the gateway and agent processes."""
        for proc, name in [
            (self.gateway_proc, "gateway"),
            (self.agent_proc, "agent"),
        ]:
            if proc is not None:
                try:
                    proc.terminate()
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()

        self.gateway_proc = None
        self.agent_proc = None

        # Clean up socket
        socket_path = Path(self.SOCKET_PATH)
        if socket_path.exists():
            try:
                socket_path.unlink()
            except OSError:
                pass

    def is_running(self) -> bool:
        """Check if gateway and agent are running."""
        if self.gateway_proc is None or self.agent_proc is None:
            return False
        return self.gateway_proc.poll() is None and self.agent_proc.poll() is None
