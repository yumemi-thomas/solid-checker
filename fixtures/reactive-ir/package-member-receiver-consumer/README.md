# Direct object members and optional reads

Hand-stated synthetic contract extending `package-effectful-member-consumer` (same manifest and authorization bytes; adds `idle`/`opaque` controls; not a published package): `createPanel().stop` states a mandatory caller-context read, `value` an optional one. `MandatoryInBody` (and parenthesized callee) is a violation; optional reads in a body are uncertifiable; JSX, handlers, `NoRead`, shadowed receivers are clean; mutable receivers, escapes, exports, computed selections, wrappers and member escapes are `reactive-dispatch-unresolved`.
