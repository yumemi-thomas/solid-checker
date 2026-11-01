# read-handed-in-callback

ADR 0535: full-6 (2026-11-01) certified 2,094 STRICT_READ_UNTRACKED misuse programs clean; rc.17 warns on each (fast runtime, confirmed). A read in a named function handed by name to an inline carrier (ADR 0531) inside a primitive's callback (a root's body, an effect apply) skipped ADR 0531, because `inside_non_component_function` answers for the callback around it, and took the silent nested-callback role. It is now placed at the carrier: RootForEach and ApplyCall are violations.

A function written as a member's value (`o.m = () => …`, `this.m = …`) runs wherever the member is called, which nothing follows: ReturnedMember and RootThisMember are obligations, never silent. RootProjectCarrier (a project object's `forEach`) is never a violation. HandlerForEach and RootTimeout stay legal.

The stub reduces the published rc.17 signal, root and effect declarations. The complete source also type-checks against published solid-js rc.17 with strict ES2022 settings. Package bytes are not copied here.
