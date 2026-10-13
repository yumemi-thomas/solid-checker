# 0023 — Retaining a value does not establish callback invocation

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

After ADR 0022 the Node graph reaches seroval 1.5.6 createReference. Its exact
published implementation stores both inputs in Maps and returns the value.
The generator nevertheless proposes queued invocation of both parameters;
native callable-path verification correctly refuses demand
sha256:6e39d8dad6550e9cbf23380e0076039d4de663341c05c623832c10e61c99cf92.

Separate retained values from deferred callbacks in the shared runtime argument
classification. Collection insertion and a parameter property of a retained
constructed object establish storage only. They must open callback knowledge,
not emit a queued invoke operation. Ordinary direct calls and audited runtime
schedulers retain their own classifications. Do not change the native proof
requirement to accept a stored value as callable or invoked. No sandbox policy,
published bytes, accessor census or probe disposition changes.

Use a reduced Map/Set/Array storage fixture with a direct invocation sibling.
Verify the generated unknown state and reject a transplanted direct callback
claim. Measure the real Node graph again; a later proof refusal remains a
refusal rather than a reason to weaken verification.
