# Source callable identity in leaf scopes

The leaf checker consumes the same exact callable transport as source read
analysis. Returned arrows and receiver-inert shorthand methods retain their
implementation identity, including across files. Their bodies must still be
walked for forbidden operations; a clean read summary is not a leaf proof.

`CleanMethod` and `CleanArrow` are clean. `ForbiddenMethod` and
`NestedPrimitive` prove forbidden operations. Conditional/default execution
remains uncertain. Receiver-dependent, overwritten and escaped methods remain
uncertifiable in `leaf-source-call-targets-negative`, keeping its mutation and
escape inventory separate from the positive controls. Independent retained
mutation tests pin writes to different fresh allocations versus opaque targets.
Mutation and escape checks remain mandatory. In `SelectedInApply`, the
number `next` handed to `cluster.select` keeps ADR 0320's unmodeled-use
obligation: this project is not a closed program, so the apply callback's
producer is not followed and only its declared type says it is a primitive. The stub's callback return shapes match published
Solid RC.13 declarations, against which these sources are also type-checked.
