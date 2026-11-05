SC3001 attributes a leaf operation to a primitive only when the binder confirms the callee is that primitive (ADR 0711). Where
it does not, the finding is uncertifiable with blocker `leaf-operation-identity-unproven`, written in the callback or reached through
a helper (ADR 0672). The first effect is the proved control.
