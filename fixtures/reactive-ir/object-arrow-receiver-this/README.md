# object-arrow-receiver-this

**Claim (ADR 0581).** An object literal's arrow slot is receiver-inert whatever its text says: an arrow binds no `this`, `arguments` or `super` of its own, so a nested class's `this.m = …` (or any other such spelling in it) names the enclosing scope's, never the object. `box.cb` then resolves to the arrow exactly (`receiver_inert`, `literal_function`); `eval` and escapes stay refused for every function, and a method or function value spelling `this` still is.

`AfterAwaitAssignThis`: SC1002 uncertifiable (the async compute resolves; the member a class assigns after the await is ADR 0565's hand-off). `AfterAwaitDirect`: SC1002 violation (a direct read after the await in the resolved arrow). `MethodThisRefused`: a method whose `this` is the object stays unresolved (no SC1002 through `box.cb`).

The effect-apply slots of the same shape are cases of `rc13-flush-in-effect-callback` (`classBox`, SC2008 uncertifiable) and `primitive-in-effect-callback` (`MemoInClassMemberOfSlot`, SC6002 uncertifiable), whose package installs those rules need.

Runtime: fuzzer mx-pairs `handoff-assign-this` × `cb-bundle` (UNTRACKED_READ_AFTER_AWAIT memo-async/effect-async 14, FLUSH_IN_EFFECT_CALLBACK 5, PRIMITIVE_IN_EFFECT_CALLBACK 15 false cleans, silent at the site before this change). Stub: `async-carrier-call-attributed`'s. `tsc --noEmit` clean against the published rc.17 typings (tsrs).
