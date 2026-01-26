#!/bin/bash
# Test: Use the custom /review command
# Expected: brainpro executes the custom command from .claude/commands/

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

check_brainpro_binary

setup_test "custom-command"

# Reset mock_webapp scratch
reset_mock_webapp

# Use the custom review command
# Note: This tests if brainpro can read and execute custom commands
OUTPUT=$(run_brainpro_in_mock_webapp "Read .claude/commands/review.md and follow its instructions to review this codebase.")

# Assert brainpro did some kind of review based on the command
assert_output_contains_any "$OUTPUT" "security" "deprecated" "documentation" "TODO" "review"

cleanup_mock_webapp
report_result
