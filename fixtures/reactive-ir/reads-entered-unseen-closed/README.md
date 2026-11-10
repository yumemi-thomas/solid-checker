# reads-entered-unseen-closed

**Claim (ADR 0203, 0934).** `reads-entered-unseen` in a closed program (`.solid-checker/runtime.json`): no unseen code imports the exports, so an export nothing calls never runs and one a handler calls runs on its event. Clean. Stubs: `rc21-listener-no-dispatch`'s.
