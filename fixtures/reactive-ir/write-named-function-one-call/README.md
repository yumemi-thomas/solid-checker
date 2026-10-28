# write-named-function-one-call

Claim (ADR 0479): a function bound to a name whose only reference is one call in its file runs on that call's stack, and the climb follows that call: a signal write in such a closure called once in a memo is SC2001 (rc.17 throws), and one called once in a handler is clean (the walk enters the handler, not the component body). Not proven in place: a named function expression (also entered by its own name), a closure another reference passes on, an exported function. Stubs as in `write-named-closure-in-root`.
