#!/bin/sh
# PreToolUse guard for Bash: snapshot updates record a deliberate semantic
# change, so force a confirmation prompt that restates the rule.
# Everything else passes through untouched (empty output, exit 0).
command=$(jq -r '.tool_input.command // empty')
case "$command" in
  *coverage.mjs*--update*|*coverage-update*)
    printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"A snapshot update records a deliberate change: `make coverage` must already have shown that every moved finding is intentional (AGENTS.md, invariant 6)."}}'
    ;;
esac
exit 0
