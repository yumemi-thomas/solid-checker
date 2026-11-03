# async-creation-pass-forms

**Claim (ADR 0570).** The fuzzer's lg-pairs later-mount false positives (79, Chrome-confirmed: 23 body, 23 root, 33 `onSettled`) in fixture form: a context-handed memo read in a component body, a `createRoot` callback or `onSettled`, through an alias, a destructured slot, a local hook's member or directly, inside an IIFE, a module helper (`helpers.ts`) or a local closure.

Violations (`FirstParent`, the provider its static child, rendered by `main.tsx`): SC5001 in every `First*` reader (`onSettled` ones at warning severity), except `FirstBodyDirectClosure` and `FirstRootDirectClosure`, whose closure reads keep their earlier obligations (an opaque options row; the root's read in a closure).

Uncertifiable, never a violation (each a violation on 719b0e49a): SC5001 in every `Later*` reader, which the button mounts after the memo settled; rc.17 warns only `STRICT_READ_UNTRACKED` there.

Stub: `async-creation-pass`'s. `tsc --noEmit` clean on the stub and on the published rc.17 packages.
