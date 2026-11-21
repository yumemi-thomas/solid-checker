# reaction-closure-read

**Claim (ADR 1176, N8).** A read in a layer that runs in place (a local closure the callback calls, an IIFE;
`write_roles::climb_in_place_layers`) inside a callback that reports untracked reads (a `createReaction` effect) takes the role at
the layer's call in that callback, when that role reports: SC1001 is a violation, where the read was uncertifiable
(`callee-callback-timing`: the closure was placed in the component body, which its calls do not prove). rc.21 warns
STRICT_READ_UNTRACKED when the tracked signal changes.

Violations: `ClosureInReaction`, `IifeInReaction`. Clean: `ClosureInTracking` (the tracking function tracks). Chrome:
`runtime-oracle/reaction-closure-read`. Stub: `action-step-window`'s.
