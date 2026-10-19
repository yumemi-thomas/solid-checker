# Unclosed source method receivers

Receiver use, mutation, opaque handoff and a sibling that leaks its holder all remain uncertifiable; this project keeps
mutation and escape controls independent from `leaf-source-call-targets`'s closed method bodies. An asserted construct
signature cannot make a shorthand method's body execute as a constructor and must never produce a proven leaf violation.
