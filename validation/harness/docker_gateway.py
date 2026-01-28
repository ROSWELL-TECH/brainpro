"""Docker compose gateway management."""

import os
import subprocess
import time
from pathlib import Path
from typing import Optional
import urllib.request
import urllib.error


class DockerGateway:
    """
    Manages Docker compose lifecycle for brainpro gateway.

    Handles docker-compose.override.yml generation for API keys,
    container startup, health checks, and cleanup.
    """

    GATEWAY_PORT = 18789
    HEALTH_URL = "http://localhost:18789/health"
    WS_URL = "ws://localhost:18789/ws"

    # Map of env var name -> secret file name
    API_KEY_MAP = {
        "VENICE_API_KEY": "venice_api_key",
        "OPENAI_API_KEY": "openai_api_key",
        "ANTHROPIC_API_KEY": "anthropic_api_key",
    }

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.secrets_dir = project_root / "secrets"
        self.override_file = project_root / "docker-compose.override.yml"

    def start(self, timeout: int = 60) -> str:
        """
        Start Docker services.

        Args:
            timeout: Max seconds to wait for health check

        Returns:
            WebSocket URL for the gateway

        Raises:
            RuntimeError: If services fail to start or health check times out
        """
        self._setup_secrets()
        self._clean_workspace()

        # Start Docker services
        result = subprocess.run(
            ["docker", "compose", "up", "-d", "--build"],
            capture_output=True,
            text=True,
            cwd=self.project_root,
        )
        if result.returncode != 0:
            raise RuntimeError(f"Docker compose up failed: {result.stderr}")

        # Wait for health
        if not self._wait_for_health(timeout):
            self.stop()
            raise RuntimeError("Gateway failed to become healthy")

        return self.WS_URL

    def _setup_secrets(self) -> None:
        """Generate secrets files and docker-compose.override.yml."""
        self.secrets_dir.mkdir(parents=True, exist_ok=True)

        # Generate gateway token if needed
        gateway_token_file = self.secrets_dir / "brainpro_gateway_token.txt"
        if not gateway_token_file.exists() or gateway_token_file.stat().st_size == 0:
            import secrets as secrets_lib

            token = secrets_lib.token_hex(32)
            gateway_token_file.write_text(token)
            gateway_token_file.chmod(0o600)

        secrets_yaml = []
        service_secrets = []

        for env_var, secret_name in self.API_KEY_MAP.items():
            secret_file = self.secrets_dir / f"{secret_name}.txt"
            value = os.environ.get(env_var, "")

            # Write env var to secret file if set
            if value:
                secret_file.write_text(value)
                secret_file.chmod(0o600)
            elif not secret_file.exists():
                secret_file.touch()
                secret_file.chmod(0o600)

            # Add to override if file exists
            if secret_file.exists():
                secrets_yaml.append(
                    f"  {secret_name}:\n    file: ./secrets/{secret_name}.txt"
                )
                service_secrets.append(f"      - {secret_name}")

        # Generate override file
        if secrets_yaml:
            override_content = f"""services:
  brainpro:
    secrets:
{chr(10).join(service_secrets)}
secrets:
{chr(10).join(secrets_yaml)}
"""
            self.override_file.write_text(override_content)
        else:
            if self.override_file.exists():
                self.override_file.unlink()

    def _clean_workspace(self) -> None:
        """Clean workspace directory (handles container-owned files)."""
        workspace_dir = self.project_root / "workspace"
        scratch_dir = self.project_root / "fixtures" / "scratch"

        try:
            subprocess.run(
                [
                    "docker",
                    "run",
                    "--rm",
                    "-v",
                    f"{workspace_dir}:/ws",
                    "-v",
                    f"{scratch_dir}:/scratch",
                    "alpine",
                    "sh",
                    "-c",
                    "rm -rf /ws/* /ws/.[!.]* 2>/dev/null; chown -R 1000:1000 /ws; "
                    "rm -rf /scratch/* /scratch/.[!.]* 2>/dev/null; chown -R 1000:1000 /scratch",
                ],
                capture_output=True,
                timeout=30,
            )
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass  # Best effort cleanup

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
        """Stop Docker services and cleanup."""
        subprocess.run(
            ["docker", "compose", "down"],
            capture_output=True,
            cwd=self.project_root,
        )

        # Remove override file
        if self.override_file.exists():
            self.override_file.unlink()

    def is_running(self) -> bool:
        """Check if Docker services are running."""
        result = subprocess.run(
            ["docker", "compose", "ps", "--format", "json"],
            capture_output=True,
            text=True,
            cwd=self.project_root,
        )
        return "brainpro" in result.stdout and "running" in result.stdout.lower()
