# Package callback-arguments consumer

The consumer side of `fixtures/package-contracts/callback-reactive-arguments`.
A callback row's `arguments` descriptors claim "this helper hands your callback
a reactive value at parameter N". Source discovery materializes such a claim in
exactly one shape — an inline function literal whose span is the argument,
carrying an `accessor` descriptor. Every other schema-valid shape used to be
dropped silently, which analyzed the callback body as if the contract had said
nothing about its arguments: the fail-open direction.

- `inlineLiteral` is the bindable shape. The claim is applied and the call
  reports nothing.
- `inlineLiteralIgnoringTheArgument` is a *restless arrow* declaring fewer
  parameters than the contract describes. Only that combination proves
  blindness: an arrow has no `arguments` object and no rest binding, so there is
  no expression in its body that could name the described argument. The
  unbindable descriptor beyond its parameters is a proof, not a gap.
- `byName` passes the callback by reference. Nothing binds, and the call must
  produce one `package-contract-incomplete` uncertifiable finding at the
  argument.
- `storePathDescriptor` passes an inline literal that does declare the
  described parameter, but the descriptor kind is `store-path`, which the
  consumer does not materialize. Same fail-closed obligation.
- `restParameterAbsorbsTheDescriptor` declares no ordinary parameter at all and
  still reads the described argument, because a rest parameter is not one of
  `parameters` and absorbs every argument from that index onward. A short
  parameter list is not by itself a proof of blindness.
- `argumentsObjectObservesTheDescriptor` declares no parameter either and reads
  the described argument through the `arguments` object of a non-arrow function
  expression. Same fail-closed obligation.

What this fixture pins is the *demand* side: which call sites keep the claim
and which fail closed. That the bound claim really becomes an accessor is
pinned on the producer side by
`contracts_process::package_generator_describes_reactive_callback_arguments`;
a materialized callback-parameter accessor has no locally reportable read of
its own, because a read inside an unclassified callback is already suppressed.

The declarations are exact for this fixture package; every finding depends on
the runtime contract, not on trusting the declaration as runtime evidence.
