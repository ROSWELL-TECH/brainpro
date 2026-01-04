#!/bin/bash
# Test: Default mode blocks writes without --yes
# Expected: File is NOT created

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../lib/common.sh"
source "$SCRIPT_DIR/../../lib/assertions.sh"

setup_test "deny-write"
reset_scratch

# Run WITHOUT --yes flag - call binary directly to avoid auto-approval
# Pipe 'n' to decline any permission prompt
PROMPT="Write 'test' to fixtures/scratch/blocked.txt"
echo "Command: $YO_BIN -p \"$PROMPT\" (no --yes)" >> "$TEST_LOG"
OUTPUT=$(echo "n" | "$YO_BIN" -p "$PROMPT" 2>&1)
EXIT_CODE=$?
echo "Exit code: $EXIT_CODE" >> "$TEST_LOG"
echo "Output:" >> "$TEST_LOG"
echo "$OUTPUT" >> "$TEST_LOG"
echo "---" >> "$TEST_LOG"

# File should NOT exist (user declined or permission blocked)
assert_file_not_exists "$SCRATCH_DIR/blocked.txt"

report_result
