# callback-parameter-completion

**Claim (ADR 0277).** A callback passed to a project helper runs during the call only when the helper calls that parameter on every completion of its own body; behind a branch, ternary, loop or logical assignment its reads are obligations, and an accessor passed by reference keeps an uncertifiable row. `runAlways`, inline arrows and exact class instances (ADR 0279) violate; conditional/loop/`??=` calls, `never`-guards, reassigned/spread-shifted/`.call` forms (ADR 0278), patchable buses (ADR 0299) and `MaybeOverride` are uncertifiable.
