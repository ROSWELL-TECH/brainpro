#!/usr/bin/env python3
"""CLI wrapper for running brainpro validation tests.

Usage:
    ./run_tests.py                    # Run all tests in yo mode (default)
    ./run_tests.py --mode=yo          # Run in yo mode (direct binary)
    ./run_tests.py --mode=native      # Run with native gateway
    ./run_tests.py --mode=docker      # Run with docker-compose gateway
    ./run_tests.py tests/test_01_tools.py  # Run specific test file
    ./run_tests.py -k test_read       # Run tests matching pattern
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(
        description="Run brainpro validation tests",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--mode",
        "-m",
        choices=["yo", "native", "docker"],
        default=None,
        help="Execution mode (default: yo, or BRAINPRO_TEST_MODE env var)",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="count",
        default=0,
        help="Increase verbosity (-v, -vv, -vvv)",
    )
    parser.add_argument(
        "--capture",
        "-s",
        action="store_true",
        help="Disable output capture (show all output)",
    )
    parser.add_argument(
        "-k",
        "--keyword",
        help="Run tests matching keyword expression",
    )
    parser.add_argument(
        "-x",
        "--exitfirst",
        action="store_true",
        help="Exit on first failure",
    )
    parser.add_argument(
        "--tb",
        choices=["auto", "long", "short", "line", "native", "no"],
        default="short",
        help="Traceback style (default: short)",
    )
    parser.add_argument(
        "tests",
        nargs="*",
        help="Specific test files or paths to run",
    )

    args = parser.parse_args()

    # Build pytest command
    cmd = [sys.executable, "-m", "pytest"]

    # Add mode
    mode = args.mode or os.environ.get("BRAINPRO_TEST_MODE", "yo")
    cmd.extend(["--mode", mode])

    # Add verbosity
    if args.verbose:
        cmd.append("-" + "v" * args.verbose)

    # Add capture
    if args.capture:
        cmd.append("-s")

    # Add keyword filter
    if args.keyword:
        cmd.extend(["-k", args.keyword])

    # Add exit first
    if args.exitfirst:
        cmd.append("-x")

    # Add traceback style
    cmd.extend(["--tb", args.tb])

    # Add test paths
    if args.tests:
        cmd.extend(args.tests)

    # Change to validation directory
    validation_dir = Path(__file__).parent
    os.chdir(validation_dir)

    # Check if binary exists
    project_root = validation_dir.parent
    binary_path = project_root / "target" / "release" / "yo"
    if not binary_path.exists():
        print(f"ERROR: yo binary not found at {binary_path}")
        print("Run: cargo build --release")
        sys.exit(1)

    # Check for API key
    api_keys = ["VENICE_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"]
    secrets_dir = project_root / "secrets"
    has_key = any(os.environ.get(k) for k in api_keys)
    if not has_key:
        for key in api_keys:
            secret_file = secrets_dir / f"{key.lower().replace('_api_key', '_api_key')}.txt"
            if secret_file.exists() and secret_file.stat().st_size > 0:
                has_key = True
                break

    if not has_key:
        print("WARNING: No API key found (VENICE_API_KEY, ANTHROPIC_API_KEY, or OPENAI_API_KEY)")
        print("Tests may fail without an API key")

    # Run pytest
    print(f"Running: {' '.join(cmd)}")
    print(f"Mode: {mode}")
    print()

    result = subprocess.run(cmd)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
