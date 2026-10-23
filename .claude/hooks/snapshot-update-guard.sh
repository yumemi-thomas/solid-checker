#!/bin/sh
# PreToolUse reminder for Bash: a snapshot update records a deliberate
# semantic change. The rule is restated to the agent as context (AGENTS.md,
# invariant 6); it no longer blocks on a user prompt, so delegated agents
# can run the update once they have read the moved findings. Everything
# else passes through untouched (empty output, exit 0).
command=$(jq -r '.tool_input.command // empty')
case "$command" in
  *"make coverage-update"*|*coverage.mjs*--update*)
    printf '%s' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","permissionDecisionReason":"snapshot-update-guard: allowed without a prompt; the rule is restated to the agent.","additionalContext":"A snapshot update records a deliberate change: `make coverage` must already have shown that every moved finding is intentional (AGENTS.md, invariant 6). Run it once, after all fixture edits, and list every moved finding with a one-line reason in the commit message."}}'
    ;;
esac
exit 0
