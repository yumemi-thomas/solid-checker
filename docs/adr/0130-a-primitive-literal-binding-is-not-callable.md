# ADR 0130: A primitive-literal binding is not callable

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the byte recognizer (`rust/crates/solid-facts/src/ast/object_binding.rs`,
  `unwritten_primitive_binding`), the certifier's export-root fallback
  (`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`,
  `require_object_export_root`)
- Relation: widens the unwritten-object-binding premise
  (`docs/package-contract-v2/phase21/2026-09-09-devtools-remaining-frontier.md`)
  by one initializer class. Leaves ADR 0099 and the producer's refusal of
  `any`, `unknown` and `never` unchanged.

## Context

`@tanstack/router-core@1.171.22` ships `./isServer` with one variant per
condition. The browser/`import` variant is

```js
// dist/esm/isServer/client.js
const isServer = false;
const loadServerRoute = void 0;
export { isServer, loadServerRoute };
```

```ts
// dist/esm/isServer/client.d.ts
export declare const isServer = false;
export declare const loadServerRoute: never;
```

The generator gives `loadServerRoute` the `plain` shape. Its declaration answers
`Unknown` callability, but the analysis-side runtime binding census closes the
binding as non-callable (`export_kind_proof_from_entity`). The certifier then
demands `recursive-value-shape` with `NonCallable` at the export root. The
producer answers from the declared type, and `callabilityOfType` refuses `never`
on purpose: the bottom type is assignable to every function type, so it proves
nothing about callability. No fallback applied. The object-binding premise
accepts only an object literal, and the factory premise needs a call. The demand
stayed open, and the published graph refused at that node:

> Type Facts certification failed for graph node
> @tanstack/router-core@1.171.22 (./isServer) … recursive-value-shape
> (…:loadServerRoute): export root is not compiler-proved non-callable and
> non-constructable

Every `@tanstack/solid-router@2.0.0-rc.8` root case imports
`@tanstack/router-core/isServer`, directly and through router-core's own `.`. So
in the viviana-ui consumer environment (`viviana-ui-main-b005c00a`) all four root
cases refused. The tier held the package only for `./ssr/client`, and all 213
of viviana `apps/web`'s imports of the root stayed at the acceptance gate. The
generated-proposal lane's own refusal, `accepted dependency
@tanstack/router-core has no exact runtime binding for export
DEFAULT_PROTOCOL_ALLOWLIST`, is the same wall. router-core was not accepted
because this node refused.

## Decision

1. **A second initializer class for the same byte premise.** The certifier may
   discharge a root `recursive-value-shape` demand that asserts `NonCallable`
   when the verified export's exact runtime binding satisfies all of these:
   - it is a module-level lexical or `var` declarator in authenticated
     JavaScript ESM bytes of the node's own snapshot;
   - it is found through the exact binder edge from the export span;
   - it is never written and never redeclared, and nothing in the module
     references `eval`;
   - its initializer, with transparent wrappers removed, is a primitive
     literal (string, number, bigint, boolean, `null`, or a template with no
     substitutions) or `void` applied to one.

   This is exactly the object-binding rule with another initializer. It is
   recorded as the site
   `recursive-export-primitive-binding:v1:<owner>:<path>:<spans>`.
2. **Nothing is claimed about the declaration.** A `never`, `any` or `unknown`
   declaration stays unanswered, and ADR 0099's `notCallableValue` still refuses
   it. The call-domain closures of such an export are withheld as before. This
   premise closes only the root shape claim.
3. **Refused on purpose:**
   - the identifier `undefined`, because it names a binding and a module can
     shadow it;
   - `void` of anything but a literal, for example `void f()` or `void x`;
   - unary arithmetic such as `-1`;
   - a template with substitutions;
   - any write, destructuring write, redeclaration or `eval`.

   Each of these keeps the open demand.

## Soundness

Each accepted initializer evaluates to a primitive and has no observable
effect. A binding that is never written therefore holds that primitive, or is
in its TDZ, or holds `undefined` before a `var` is initialized. None of these
has `[[Call]]` or `[[Construct]]`. That is the whole of the claim. The
recognizer binds its answer to the SHA-256 of the complete source and the exact
span, so changed bytes or a neighbouring span do not match. The owner, runtime
path and span are the verifier's replayed export binding, as for objects, so a
sibling version's bytes or a declaration file cannot substitute. The claim is
also true when the declaration says `never`, because the value the runtime
binding really holds is `undefined`.

Pinned by `object_binding::tests::primitive_binding_*` (the recognizer, with
its refusals) and by the rows added to
`object_export_root_receipt_requires_an_unwritten_authenticated_binding`: a
`never` declaration over `void 0` and over `false`, an `unknown` declaration
over a string, and three `never` refusals (a later write, `undefined`, and
`void globalThis.f()`). With the primitive branch disabled, the first new row
fails. That run was made.

## Consequences

Measured on 2026-09-27 with the release binary. The run was
`consumer-environment-runs` for `viviana-ui-main-b005c00a` only. The tier and
the census were not regenerated.

| | before | after |
| --- | ---: | ---: |
| solid-router certified entrypoints | 1 of 4 (`./ssr/client`) | 2 of 4 (`.`, `./ssr/client`) |
| `rootCertified` | false | true |
| root exports in the accepted contract | 0 | 97 of 97 |
| certified closures (all graph nodes) | 0 | 106 (seroval 30, seroval-plugins 32, cookie-es 2, …) |
| withheld closures | 0 | 288 |

All ten root names that viviana imports are published: `createFileRoute`
(181 sites), `Link` (12), `Outlet` (8), `useLocation` (5), `redirect` (2), and
`useNavigate`, `createRouter`, `createRootRoute`, `HeadContent` and `Scripts`
(1 each). Each is `shape: callable` with an empty `call`. The generator proposed
no closure for solid-router's own exports, so every domain is open. The
*estimated* consumer effect, once the lead regenerates the tier: the 213 import
sites move from the acceptance gate to the open-claims gate. None certifies. No
consumer sweep was run to measure this.

`./ssr/server` still refuses for two reasons. The first is
`runtime-library-policy-required: node:stream`. The second is the external
`export *` from `@tanstack/router-core/ssr/server`. This ADR addresses neither.
