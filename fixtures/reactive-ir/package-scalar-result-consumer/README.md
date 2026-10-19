# Single primitive callback completion

ADR 0262: `takeResult`'s optional result call (exact callableOnly gate, closed uses census) cannot be entered by a runtime primitive, so `SyntaxClean`/`BuiltinClean` carry no result-dispatch obligation; `ProducerViolation` keeps its SC2001 violation; refused completions, shadowed `Date`/`Math`, replaced locals and the `readResult`/`unknownResult`/`callTrackedResult` guards keep uncertifiability. Stubs and JSX are byte copies of `root-callback-return` (rc.9).
