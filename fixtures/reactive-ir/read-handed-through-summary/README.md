# read-handed-through-summary

ADR 0536: full-6 (2026-11-01) left 562 rc.17 STRICT_READ_UNTRACKED warnings certified clean after ADR 0535. A helper's read reached an inline carrier or a callback prop only through a call: `wrapUse()` in a named function handed to `[1].map(hf)`, or `f()` / `wrapUse()` in a component's prop literal. The summary path returned silent where the direct-read path places the read. BodyMap, RootForEach, ApplyCall, PropHelper and PropClosure are violations.

HandlerMap, RootTimeout, TrackedCompute and PropHandler stay legal. ProjectCarrier (a project object's `forEach`), PropKept (a consumer that keeps the prop and never calls it) and MemberValue (a function written as a member's value) are obligations, never silent.

The stub reduces the published rc.17 signal, memo, root and effect declarations. The complete source also type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.
