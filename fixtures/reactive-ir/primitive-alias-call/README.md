# A call of a primitive through a name no rule reads (ADR 0884)

The compiler resolves `held(…)` to `onCleanup`, and the primitive rules read names, so
no rule would see the operation. It gets SC9012 `primitive-alias-call` at the call, plus
a U of each rule that primitive answers to, since a code with no site line is covered
only by its own rule's row. Direct calls get neither.
