#!/bin/bash
# Test: Ask brainpro to review code for quality issues
# Expected: brainpro identifies TODO comments, deprecated code, etc.

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

check_brainpro_binary

setup_test "code-review"

# Reset mock_webapp scratch
reset_mock_webapp

# Ask brainpro to do a code review
OUTPUT=$(run_brainpro_in_mock_webapp "Review the codebase for code quality issues. Look for TODO comments, deprecated functions, missing documentation, and code smells. Summarize your findings.")

# Assert brainpro found some issues (TODO, deprecated, undocumented, etc.)
assert_output_contains_any "$OUTPUT" "TODO" "deprecated" "undocumented" "documentation" "old_query"

cleanup_mock_webapp
report_result
