#!/bin/sh
# PreToolUse guard for Bash: snapshot --update runs record a deliberate
# semantic change, so force a confirmation prompt that restates the rule.
# Everything else passes through untouched (empty output, exit 0).
command=$(jq -r '.tool_input.command // empty')
case "$command" in
  *coverage.mjs*--update*|*parity.mjs*--update*)
    printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"Snapshot --update records a deliberate change: the non-updating comparison must already have shown that every moved finding is intentional (AGENTS.md, Known traps)."}}'
    ;;
esac
exit 0
