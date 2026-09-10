# ADR 0055: Complete imported union premises

Status: implemented; full-corpus measurement and handoff verification passed.

## Measured problem

ADR 0054 exposed six `buildHTMLStyles` candidates refusing a template
coercion in `buildTransform`. Its helper twin could not resolve parameter 2:
the demanded `TransformTemplate | undefined` became `any`.

The focused regression reproduces the mechanism with `Axis | undefined`.
Before this fix, the caller recorded an empty spelling and only the outer
union flag as its identity. The unpremised helper records an actual coercion,
so the regression is not vacuous.

## Decision and verifier binding

Protocol 38 spells an unaliased union only when **every** constituent can be
spelled: an exact exported declaration reference through an import type, or a
type the compiler explicitly establishes as non-object. Constituents are
parenthesized and none is omitted. An unsupported member leaves the complete
spelling unavailable. Named aliases retain the existing exact exported-alias
spelling.

Union identity now includes the complete multiset of constituent identities,
sorted independently of compiler-internal type IDs. Each constituent retains
its type flags and named declaration/alias identity. The outer union flag
alone cannot distinguish same-named types from different modules.

The existing consumer still binds the caller's exact argument premise to
the helper demand and requires byte-for-byte echoes of type text, identity
and spelling. The producer rechecks the generated annotation's type and
identity before emitting an echoed premise. The receipt names the helper
premise and binds its transcript digest. A spelling remains a resolution
hint, never authority to change the type.

## Tests and boundary

The producer test failed before the fix and passes after it. It verifies the
original coercion exists, the complete optional import spelling is recorded,
the helper echoes it and its numeric coercion clears. Substituting null for
undefined refuses. Substituting a structurally identical, same-named Axis
from a different declaration file also refuses, testing constituent identity
in addition to printed text. Existing single-import and primitive helper
premise tests pass.

The packed native fixture closes the numeric optional Axis case through the
live producer, creates census, synthesized veto and receipt finalizer. An
Axis whose members are unknown keeps its coercion and stays withheld.

This does not relax unknown types, inaccessible declaration references,
generic spelling requirements, or the helper's complete execution census.
Nor does a repaired helper premise itself prove every operation in that
body.

## Full-corpus result

All 418 probes were measured. Withheld candidates remain **462**: 442 creates
(405 census, 37 veto) and 20 returns. No candidate identities were added or
removed. Eighteen first-refusal explanations changed across three exports:
six buildHTMLStyles premises now bind but retain the same template coercion;
six removeAxisTransforms premises now bind and expose a later binary coercion;
six calcBoxDelta premises resolve to string | number | undefined but still
refuse the printed-type mismatch against AnyResolvedKeyframe | undefined.
The last case remains a producer limitation, not a package defect, and needs
a focused alias-preservation test before changing equality checks.

Full make verify passed in 210.39 seconds, exit 0, TOTAL present and no
FAILED during step marker. See the [measurement](../package-contract-v2/phase21/2026-09-07-union-census-measurement.md).
