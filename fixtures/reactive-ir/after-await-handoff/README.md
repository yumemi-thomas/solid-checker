# after-await-handoff

**Claim (ADR 0565).** A function the after-await walk (`reactive-read-after-await`, SC1002) does not place, handed where nothing here follows it (a project or unresolved call's whole argument, a member's value, an object literal's slot, a method, a copy), may run on the continuation when it is written after the computation's await, handed on after it, or (a member's value, a slot, a method) outlives a later await: its reads are proof obligations, never a silent clean. One hand-off proves: a same-file wrapper called by its plain name that calls the function (its parameter) on every completion of its own synchronous body (`helper_argument_run`) runs it on the call's stack.

Violations (each silent on 719b0e49a): `Wrapper`, `ModuleWrapper`, `HandedAfterAwait` (written before the await, handed on after it), `Member` (ADR 0895: a closed literal's slot naming the function, called past the await).

Uncertifiable, never a violation (each silent on 719b0e49a): `WrapperConditional` (the wrapper calls it on no path here), `WrapperDefers` (a timer inside the wrapper), `WrapperStores` (a local `runNow` shadowing the module's keeps it), `Namespace` (a wrapper in another module), `MemberWrapper` (a closed literal's member), `Assigned`, `AssignedThis`, `ClassMethod`, `ShorthandEscaped` (a method reached through a copy of its object), `LiteralHandOff`, `AssignedBeforeAwait`.

Violation already on 719b0e49a (ADR 0351): `SlotBeforeAwait`, a closed object literal's member called after the await.

Clean: `Timer` (a fresh stack), `NeverCalled`, `Returned` (the computation resolves with it: it runs, if ever, after the continuation), `BeforeAwait` (run before the await: tracked), `Tracked` (the source was read before the await, ADR 0195).

Runtime: rc.17 warns `UNTRACKED_READ_AFTER_AWAIT` for the fuzzer's `handoff-wrapper`, `handoff-member` and `handoff-assign*` programs (mx-recall, full-main-644bd1a), the shapes of `Wrapper`, `Member`, `Assigned`, `AssignedThis`. Stub: `async-carrier-provably-runs`'s. `tsc --noEmit` clean against the stub and against the published rc.17 typings.
