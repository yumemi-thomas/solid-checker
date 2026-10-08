# Direct object members and optional reads

This hand-stated synthetic contract fixture extends
`fixtures/reactive-ir/package-effectful-member-consumer`. The package manifest
and authorization bytes are identical to that fixture; the declaration and
contract alone add the synthetic `idle` and `opaque` controls. This fixture
does not claim these operations describe a published package.

`createPanel().stop` states a mandatory caller-context read, `value` states an
optional 0..1 caller-context read, and `idle` states no reads. `opaque` states
callability only. The factory closes callbacks/reads/creates/returns; writes
remain open. Tuple behavior retains the original contract.

Intended assertions (not checker observations):

- MandatoryInBody: strict-read-untracked **violation** at `panel.stop()`.
- OptionalInBody and DestructuredOptional: strict-read-untracked
  **uncertifiable**, never violation.
- OptionalInJsx, MandatoryInJsx, NoRead, MemberInHandler,
  DestructuredOptionalInJsx: no member dispatch or strict-read finding.
- MutableReceiver, every whole-receiver escape, exported receivers,
  mutations, opaque calls, computed selections and wrapped initializers:
  reactive-dispatch-unresolved **uncertifiable**, no bound member violation.
- WrappedCallee, SatisfiesCallee, WrappedReceiver and MemberEscape:
  reactive-dispatch-unresolved **uncertifiable**.
- ParenthesizedCallee: `(panel.stop)()` is the same call as `panel.stop()`
  (parentheses keep the receiver), so it is the same proven **violation**.
- ShadowedReceiver: the inner parameter must never inherit the outer graph;
  the outer optional JSX read stays clean.
- TupleSelection: existing computed tuple member obligation remains.

Every receiver escape has a direct stop call beside it to catch partial
binding that would clear the obligation while still publishing a violation.
ReturnedReceiver itself is not a component: its escape must be diagnosed at
the factory, irrespective of the function's execution role.

This research tree has no coverage snapshot and no acceptance catalog.
The lead must authorize a copied fixture, compare with the fresh pinned
debug checker, then record only the reviewed snapshot. If promoted into
fixtures/reactive-ir/, add the fixture's node_modules exceptions to .gitignore.
