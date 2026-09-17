# 0024 — Propose only unchanged parameter identities

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

The final three-row run exposes an old false return relation in alpha's
roundToStepPrecision. It mutates the parameter before returning it. ADR 0016's
new exact identity check refuses demand
sha256:9fd82970472b2d2de83b0af700b64885db37b0c39e58c49550160a1b1eeb201d.
Previously a closed numeric type discharged a parameter-identity claim without
proving identity. Restoring that acceptance would be unsound.

Restrict the generator's argument-return proposals to exact ordinary unchanged
parameter symbols. Withhold the relation for any assignment (including nested
writes and updates), aliases, defaults, destructuring, async/generator wrapping,
duplicate parameter names, or eval/arguments mentions. Conservative withholding
does not prove a different output shape or absence of a return. The native
identity check stays unchanged. No census or sandbox policy changes.

Populate the existing normalized parameter initializer field for ordinary and
arrow functions. The syntax owner was dropping this fact, so the generator
could not distinguish a defaulted parameter from a plain input. No new public
fact field or wire protocol is introduced. The native producer already records
and rejects defaults independently.
AST cache schema 41 advances to 42 to invalidate cached facts that omitted
parameter initializers.

Pin a direct identity beside mutation, update, nested mutation, alias, default,
destructuring and async controls. Measure alpha again with the same recipes.
