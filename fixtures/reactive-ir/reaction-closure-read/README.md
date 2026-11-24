# reaction-closure-read

**Claim (ADR 1202, the N8 part of ADR 1176).** A read in a layer that runs in place (a local closure the callback calls, an IIFE;
`write_roles::climb_in_place_layers`) inside a callback that reports untracked reads (a `createReaction` effect) takes the role at
the layer's call in that callback, when that role reports: SC1001 is a violation, where the read was uncertifiable
(`callee-callback-timing`: the closure was placed in the component body, which its calls do not prove). rc.21 warns
STRICT_READ_UNTRACKED when the tracked signal changes.

Violations: `ClosureInReaction`, `IifeInReaction`. Clean: `ClosureInTracking` (the tracking function tracks). Chrome:
`runtime-oracle/reaction-closure-read`. Stub: `action-step-window`'s.

`NamedHelperInReaction` (a violation on main too) pins the agreement: its read keeps the body's lexical role while its call takes the effect's role through this arm, and the named-helper timing proof (`named_helper_runs_during_body`) accepts a call placed in N8's shape (`untracked_argument_layer_call`). Without that, the N8 arm turned `reaction-store-named-called-*-helper` from violations into `callee-callback-timing` obligations.
