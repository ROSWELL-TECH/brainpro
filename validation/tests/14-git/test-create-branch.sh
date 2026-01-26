#!/bin/bash
# Test: Ask brainpro to create a feature branch
# Expected: brainpro creates and checks out a new branch

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

check_brainpro_binary

setup_test "create-branch"

# Reset mock_webapp scratch
reset_mock_webapp

# Ask brainpro to create a branch
OUTPUT=$(run_brainpro_in_mock_webapp "Create a new git branch called 'feature/auth-improvements' and switch to it.")

# Assert the branch exists
BRANCH=$(cd "$MOCK_WEBAPP_SCRATCH" && git branch --show-current)
echo "  Current branch: $BRANCH" >> "$TEST_LOG"

if [ "$BRANCH" != "feature/auth-improvements" ]; then
    echo "  ASSERT FAILED: Expected branch 'feature/auth-improvements', got '$BRANCH'" | tee -a "$TEST_LOG"
    TEST_PASSED=0
else
    echo "  OK: On branch feature/auth-improvements" >> "$TEST_LOG"
fi

cleanup_mock_webapp
report_result
