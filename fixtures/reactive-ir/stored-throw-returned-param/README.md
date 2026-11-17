# stored-throw-returned-param

**Claim (ADR 1049).** A plain throw (ADR 0893: a static `dynamic` source that is a promise, `useContext` of a default-less context never provided) ends where its function runs, and two holders now say where:

- a literal handed to an identity wrapper at the parameter it returns (ADR 0346's exact wrapper, `function same(cb) { return cb }`): the call's value is the function, so `const run = same(() => …); run()` and `same(() => …)()` run it in the component body;
- a generator stepped once where it is made (`(function* () { …; yield 1 })().next()`, ADR 0521, the site before every `yield`): the body runs on that `next()` call.

Violations: `ReturnedCalled`, `ReturnedCalledDirectly` (SC2007), `GeneratorStepped` (SC4002).

Uncertifiable: `WrapperAlsoStores` (no identity wrapper), `ConditionalReturn`, `ReturnedCaught` (the call is caught), `GeneratorAfterYield`, `GeneratorStored` (the iterator is held), `ReturnedInLiteral` (`return { cb }`: open).

Chrome: `fixtures/runtime-oracle/stored-throw-returned-param`. Stubs: `stored-throw-dynamic-mount`'s. `tsc --noEmit` clean.
