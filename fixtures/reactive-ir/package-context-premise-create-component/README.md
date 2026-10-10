# A context premise broken by `createComponent`

ADR 0153 part 3; `package-context-premise-met` has the full picture. The
compiled spelling of a provider, `createComponent(RouterContext, props)`. The
context is an argument of a call other than `useContext`, which is a
provision. Expected: `SC9005`, error, uncertifiable, at the import of
`useLocation`.

## The contract and the stubs

`node_modules/@solidjs/router/solid-reactivity.json` is a hand-written
contract for three exports: `useLocation` states every call domain closed and
empty **under the context premise `RouterContext`** (ADR 0153 part 3,
`contextPremises`), `useNavigate` states the same claims with no premise, and
`RouterContext` itself states no premise. The claims are the fixture's own; the
premise is the subject. `.solid-checker/authorize-contract.json` has
`scripts/coverage.mjs` mint a policy-2 receipt over it
(`package-merged-props-consumer` explains that mechanism). Un-authorized, the
project has no accepted contract and every import of the router is `SC9005`
for that reason instead.

The router's typings are the **published** `@solidjs/router@2.0.0-next.26`
declarations, byte for byte: `package.json` and every `dist/**/*.d.ts` from the
tarball. The closure digest and integrity in both JSON files are computed over
that `package.json`.

`solid-js`, `@solidjs/signals` and `@solidjs/web` are reduced stubs at
`2.0.0-rc.9`, the audited release. Every signature the premise check and the
sources here depend on is byte-identical to the published one: `Context`,
`ContextProviderComponent`, `FlowComponent`, `FlowProps`, `Component`,
`ComponentProps`, `createContext`, `useContext` and `createComponent` from
`solid-js`, and `Accessor`, `SourceAccessor`, `Setter` and `Signal` from
`@solidjs/signals`. `EffectOptions` keeps only its `name` member, and
`@solidjs/web` declares only the `JSX` namespace and `ResponseEnvelope`, the
two names the router's declarations import from its root.

`tsc` stays silent. Every source in the six `package-context-premise-*`
fixtures was compiled, strict and loose, against the real published typings --
the tsc oracle's audited rc.9 install (`bun scripts/tsc-oracle.mjs provision`)
with the real router next.26 package beside it -- under the oracle's own
compiler options, with zero diagnostics. The fixture stubs compile with zero
diagnostics too. `<RouterContext.Provider>` is not a case: against the real
typings it is `TS2339` (`Property 'Provider' does not exist on type
'Context<RouterContext>'`), so it is TypeScript's to report.
