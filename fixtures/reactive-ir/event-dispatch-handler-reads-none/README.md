# event-dispatch-handler-reads-none

**Claim (ADR 0934).** `event-dispatch-handler-reads` with no dispatch: every handler, inline or named, and every helper it calls runs on its event's delivery, outside any strict-read window or owner. Clean. Stubs: `rc21-listener-no-dispatch`'s.
