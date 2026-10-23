# props-spread-argument

**Claim.** An object spread is a "props spread" read (every prop getter runs) only when a value the argument may evaluate to is exactly the props binding, or an inline `omit`/`merge` view of it (ADR 0337). A conditional, a logical, an object literal or a member argument is not the props object, whatever its first token names: spreading it reads none of the props' getters, and the prop reads written inside it (`props.queries`) are ordinary member reads, decided by the callers (ADR 0216). Corpus false positive fixed (ADR 0375): kui `KsqlRoute.tsx:164,170` and `ConnectRoute.tsx:103`, oracle `adjudication-props-spread-options`.

**Cases.** Clean: `ConditionalOptions` (the kui shape, `...(props.queries === undefined ? {} : { registry: props.queries })`), `LogicalOptions` (`props.queries && { … }`), `LiteralOptions` (`...{ registry: props.queries }`), `MemberOptions` (`...props.queries`). Before ADR 0375 each was a "props spread" violation, because the argument's span resolved to its first token's symbol. No caller passes `queries` reactively, so their `props.queries` reads are static.

Violations: `ReactiveQueries` has the kui shape, but one caller passes `queries={registry()}`. The two `props.queries` member reads are setup reads of a signal-backed prop. Nothing is reported at the spread.

Uncertifiable: `PropsOptions` (`...props`): a spread into an object literal is not among ADR 0337's inert uses, so the props root is not proven current. `ConditionalProps` (`enabled ? props : {}`), `ConditionalOnProp` (`props.queries ? props : {}`) and `LogicalProps` (`enabled && props`) are uncertifiable too. There the props object is one branch's value, so the spread runs its getters only on that branch. The props root check also sees no use at a branch identifier, so the read stays an obligation, as `...props` does. `ConditionalOnProp` was a violation before, and `ConditionalProps`/`LogicalProps` were silent: their first token is `enabled`.

The stub declares only `createSignal`; the fixture type-checks with `tsc --noEmit`.
