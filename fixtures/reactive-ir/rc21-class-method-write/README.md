# SC2003: a store's class method, and `Reflect.deleteProperty`

A method a store path holds runs with the read proxy as `this`, so `this.n++` warns
`STORE_WRITE_OUTSIDE_SETTER`: an obligation, since the member holds what the store holds
at the call. `Reflect.deleteProperty(s, k)` warns too (ADR 0638). Chrome, rc.21.
