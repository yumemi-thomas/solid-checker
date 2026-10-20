# rc13-flush-in-effect-proxy-traps

SC2008 on a `flush()` that follows a property operation on a Proxy built from a literal in the same file. A `get` or `set` trap that cannot return makes the read or write not complete normally, so the `flush()` after it is not reached: silent. A trap that returns, and an object that is not the proxy, complete normally: reported. A property operation on an object whose accessors cannot be resolved stays under the feasible-path premise (assumed to complete normally; see the rule page).
