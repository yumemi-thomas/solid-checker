# Declaration imports and executable closure

ADR 0010's pair. Both packages publish the same callable declaration through
an external `source-types.Callback`. Only `probe-runtime-import` loads that
dependency at runtime. The JS resolver tests read these exact fixture files;
the backend tracer builds authenticated archives and supplies an exact,
lock-bound `Callback = () => void` declaration dependency.

The declaration-only package must retain its compiler-source acquisition edge,
have no executable dependency hazard, and certify creates through the real
census and pinned probe. Missing or wrong-lock typings must fail the demanded
callability witness. The runtime sibling must retain its all-domain hazard;
forging that hazard away must fail native snapshot replay. Empty and side-effect
runtime imports are executable even though they import no named binding.

This is a certification tracer fixture, with no generated main snapshot. The
declarations describe these fixture packages; no Solid typing is stubbed.
