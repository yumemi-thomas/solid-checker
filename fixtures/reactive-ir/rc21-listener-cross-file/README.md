# Listener delivery across analyzed files

ADR 0645: `listeners.ts` registers a signal-reading function also used as a JSX
handler. `App.tsx` delivers it during a component body. Chrome rc.21 fires
STRICT_READ_UNTRACKED, but the old checker places listener-reach only on the
registration, on another file and stack. The dispatch now keeps listener-reach U.
No new violation or target/event relationship is proved.

Variants: no-registration has no registration or read anywhere and adds no U;
deferred has the same registration but a timer dispatch and adds no U at the
dispatch (the registration keeps its existing U if required by other sites).
method-read keeps addEventListener as a value: that open read keeps U at both
the read and dispatch, as the existing registration predicate requires.
The project's existential retains every prior registration/read U.

Chrome probes: scratch cases/listener-cross, positive fires
STRICT_READ_UNTRACKED; deferred and a separate EventTarget dispatch are quiet.
The fuzzer prints one source file, so this two-file shape is retained here.
