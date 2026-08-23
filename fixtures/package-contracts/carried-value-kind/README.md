# A dependency contract's own `kind: "value"`, and who is allowed to carry it

Pins the one exemption in the kind refusal, the refusal it is an exemption
from, and — the reason this fixture has three dependencies — the door the
exemption must *not* leave open.

`hostValue` is re-exported by name from a dependency with no type
declarations, and its value comes from `globalThis.host.table`. Inside *this*
package's project the specifier is therefore `any`, and Type Facts answers
`Callability::Unknown` — no closed domain, no proof either way.

- **Without `--contract`**, that is all the generator has, so it refuses the
  entrypoint. A bare `kind: "value"` summary is the maximal certified negative
  (`validate_export` bars a `value` summary from carrying even an unknown claim
  domain), and publishing one against an unresolvable type is the defect that
  made `@solid-devtools/locator@0.16.7` certify `addClickInterceptor(fn)` as
  invoking no caller-supplied callback.
- **With the dependency's own reviewed contract**, the `value` claim is not
  this project's guess: `dependency-contract.json` carries
  `evidence.kind: "reviewed"`, so a human stood behind it. Re-deciding it here
  — where the dependency's implementation is outside the project and its
  specifier is consequently `any` — would refuse exactly the entrypoint that
  already has the better answer. So that carried summary keeps its kind.

## The laundering channel, and why provenance is the gate

The exemption is an argument about **provenance**, and only two provenances
support it (`PackageContract::kind_claims_are_trusted`):

1. a contract *this generation run* produced itself from the dependency's own
   installed sources, under this exact rule — the generator passes those with
   `--generated-contract`, and
   `fixtures/package-contracts/class-expression-kind` exercises that route;
2. a contract whose `evidence.kind` records that a human or a verifier stood
   behind its claims (`reviewed`, `verified`, `trusted`, `attested`) — which is
   what `dependency-contract.json` here is.

A contract that has *neither* is a document of unknown origin. `./laundered`
pins it: `node_modules/laundered-dependency/solid-reactivity.json` is an
`inferred` contract sitting on disk, which `dependencyContracts()` discovers by
walking `node_modules/<dep>/solid-reactivity.json` upward with no flag from the
user — the repository's own distribution convention for dependency contracts.
It claims `kind: "value"` for `addClickInterceptor(fn)`, which forwards its
caller's callback to `globalThis.host.on`: exactly the
`@solid-devtools/locator` defect, in exactly the shape an earlier
solid-checker with the `Unknown ⇒ value` defect would have written. Carrying
that kind would republish the wrong claim through the one door the refusal
leaves open, so its kind is re-decided here like any local claim — and the
dependency has no typings, so the honest answer is the refusal.

Re-deciding has to *decide*, not only refuse, which is the other half of
`.`. `laundered-typed-dependency` ships the same unreviewed `inferred`
contract calling `addTypedInterceptor` a `value`, and also ships declarations.
This project can prove that kind, so the wrong carried negative is **corrected**
to `function` with `callbacks: {"status":"unknown"}` rather than refused, and
the entrypoint survives.

Only the `kind` claim is gated this way. Every other claim in a discovered
contract is used as before: a contract is the only evidence there is about a
package this project cannot see into.

A carried summary can still be *raised* by a local fact regardless of
provenance: a class-shaped or provably callable binding at the entry file's own
specifier turns it into `kind: "function"` with callbacks unknown. See
`fixtures/package-contracts/class-expression-kind` for the class direction and
for the local (uncarried) refusal.

This fixture has no `expected.json`: it is driven by
`package_generator_keeps_a_dependency_contracts_value_kind_and_refuses_without_it`
in rust/crates/solid-facts-backend/tests/contracts_process.rs, because the
contract corpus runs `contract generate` with no `--contract` and the point
here is the difference between the two invocations.
