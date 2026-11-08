#!/bin/sh
# PreToolUse guard for Bash: snapshot updates record a deliberate semantic
# change, so restate the rule to the agent. It never asks: a prompt stops a
# background agent until someone answers it. The diff keeps the moved findings
# reviewable. Everything else passes through untouched (empty output, exit 0).
command=$(jq -r '.tool_input.command // empty')
case "$command" in
  *coverage.mjs*--update*|*coverage-update*)
    printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"A snapshot update records a deliberate change: `make coverage` must already have shown that every moved finding is intentional (AGENTS.md, invariant 6)."}}'
    ;;
esac
exit 0
