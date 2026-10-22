A member read off a server function (`loadUser.keyFor(x)`) is not a client call
to that function. Its leading token names the object, so before ADR 0346's exact
identity reads the checker took the call as a call of the server function and
reported a `Date` argument as an SC7007 violation, for the typed member too.
Here the typed member `keyFor(when: Date)` is a declaration TypeScript resolves
(the wrapper's own method), so it is silent: the Date never travels to the
server. The untyped member (`keyFor: any`) resolves to no declaration, so
whether its call crosses the transport is unproven: an SC7007 uncertainty on
the argument, never a violation (fail closed).
