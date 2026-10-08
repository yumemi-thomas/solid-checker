# ADR 0263: a package may read a fresh plain object a callback returns

- Status: accepted and implemented (2026-10-09).
- Owner: `callback_result_completion_is_plain_data` in `local_access.rs`.

`createDerivedStaticStore` calls its callback and reads keys of the object it
returns (`get-derived-key`, a Get use in the contract's callback-result
census). The consumer accepted Get only on a primitive result. So
`createDerivedStaticStore(() => ({ value: count() }))` kept a
`reactive-dispatch-unresolved` obligation even though reading a data property
of that object runs no caller code.

Get is now discharged for a fresh, complete object literal, under these
conditions:

- the existing return-structure fact must be complete: only `key: value` data
  properties, unique static identifier or string keys, identifier shorthand,
  and no getter, setter, method, spread, computed key or `__proto__`;
- the runtime kind must be Object, so arrays refuse;
- the Get must happen at the call, on the same stack, which matches the
  existing enumeration boundary. Later, queued or external Gets refuse even
  for a non-escaping result: proving a retained object unmutated is not done
  here;
- every use in the census must have an empty path. A use on a property's
  value (calling or iterating it) refuses.

`{}` has no structure fact and is refused. Values inside the object may be
anything: constructing them happens in the callback, not during the Get.

## Premise

A missing key reaches `Object.prototype`. ADR 0190 already excludes patched
built-in prototypes for every standard-library row; the same premise applies
here. Built-in accessors such as `__proto__` run no caller code.

## Measured consequences

- Fixture `package-derived-get-consumer`: plain reads are clean, and the
  producer's own write is still an SC2001 violation. Getters, setters,
  methods, spreads, computed keys, `__proto__`, arrays, `Proxy`, unknown or
  cast values, `{}`, async and named callbacks, open censuses, and uses on
  property values all keep the obligation.
- Primitives ledger, browser: 97 of 111 report correctly (was 96):
  `createDerivedStaticStore`'s write case. No correct twin has a violation.
- rc.13 corpus: no violation or uncertifiable site added or removed.
