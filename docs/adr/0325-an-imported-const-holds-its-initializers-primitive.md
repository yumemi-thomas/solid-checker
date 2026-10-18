# ADR 0325: An imported const holds its initializer's primitive

- Status: accepted and implemented (2026-10-18).
- Owners: `SemanticLookup::imported_const_is_primitive` and the cross-file arm
  of `proven_primitive` (`solid-reactive-ir/src/indexes.rs`); the digest entry
  `exported_primitive_consts` in `cross_file_proof_digest`.
- Relation: extends ADR 0320's `proven_primitive`, which read an unexported
  `const` of the same file, to a named import of another module's exported
  `const`. Both primitive consumers use it: the call-site unmodeled-use check
  (`argument_is_proven_primitive`) and the attribution discharge
  (`value_is_primitive`).

## Context

kui's tests render a screen with a constant another file exports
(`<ConnectScreen clusterId={TEST_CLUSTER} />`, `export const TEST_CLUSTER =
"quickstart"` in `testing.ts`). The component's unmodeled use of the prop is
discharged only when every tag passes a primitive, and the file-local proof saw
an import, not a constant.

## Decision

An identifier holds a primitive other than `undefined` when, exactly:

1. its declaration is the local of a **named, value** import (not a namespace,
   a default, a side-effect or a type-only import);
2. the compiler's **attested resolution** of that import's specifier lands on
   the file that declares the binding (`included_path`, else `resolved_path`):
   no re-export hop and no `export *`;
3. that file is a **module** (not a script, whose top level is global);
4. the declaration is a module-level `const` of one plain identifier, not in a
   function;
5. the module exports it under **exactly the imported name**: `export const X`
   (a declaration entry), or `export { N as X }` (a specifier entry whose
   local resolves to this declaration), neither from another module, neither
   type-only;
6. its initializer is proven primitive by `proven_primitive` in that file
   (a literal or template, an operator, a result of an audited standard-library
   function, a conditional or logical expression of those, an unexported
   `const` of that file). An object, an array, a call the audited table does
   not name, a type assertion (`as const` included) and an exported `const` of
   the declaring file refuse.

A `const` cannot be reassigned, and an ES module namespace is frozen, so
nothing changes the value after initialization.

Everything else refuses: `export let`/`var`, a default import
(`import X from`), a barrel or an `export { X } from` hop, an `export *`
collision, a namespace member (`C.X` is no identifier), a package import (its
declaration is not a program binding), an import the compiler did not attest.

## Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| the import is named | `ImportKind::Named`, value import | a default or namespace import binds something else | `Default` |
| the import lands on the declarer | attested row's landing equals the binding's file | the compiler resolved the specifier; no barrel is followed | `Barrel`, `Star`, `import_through_a_barrel_matches_cold` |
| the module is a module | `file_is_module` | a script's top-level bindings are global | covered by the existing census |
| the binding is a module-level `const` | `immutable`, identifier shape, not in a function | ES `const` and frozen namespaces | `Mutable` |
| the exported name is the imported name | declaration or specifier entry of a non-re-export | an alias is followed only through the module's own specifier | `Aliased`, `constant_file_edits_match_cold` |
| the initializer is primitive | `proven_primitive` in the declaring file | ADR 0320 | `Object_`, `Called`, `Asserted`, `Chained` |
| a cached answer is current | the set of exported primitive constants is in the cross-file digest | the proof reads another file | `constant_file_edits_match_cold` (the constant turns object, `let`, aliased, renamed and back, under a retained session, closed and open, retained equals cold) |

Self-review against the bug families: the origin is the compiler's resolved
import and the module's own export entry, never the name; keys are the exact
exported name; no wrapper is peeled (an assertion in the initializer refuses);
scripts are refused by the module test; an ES module that also names the
CommonJS exports object (`module`, `exports`, unbound, read or written) is
refused too, since a CommonJS transform can replace its export (round 12,
fixture `imported-const-commonjs`; `export =` has no named export and a CJS
script is a script); the proof reads values, not roles, so
the window, view, helper, await and mixed-role families do not apply; the only
cache risk is the other file's answer, covered by the digest and by the twin.
The twin also passes with the digest entry removed (every cache that consults
the answer is already invalidated by another input); the entry is kept as a
conservative binding rather than relied on.

## Consequences

49-project corpus against slice R's sweep (4,104 uncertifiable, 554
violations): **−3 `SC9012`** (kui `ConnectRoute.tsx:102` and `:171`, whose
`ConnectScreen` tags are the route's string and the test's `TEST_CLUSTER`;
openbot `ImageLightbox.tsx:335`, `tiles.restore(thumbnail, ZOOM_OUT_DURATION)`
with `export const ZOOM_OUT_DURATION = 190` in `lightbox-motion.ts`), 0 added,
violations unchanged: 4,104 to 4,101.

The yield is smaller than the ~20 kui rows estimated. What is left of kui's
`parameter-use-unmodeled` rows is not blocked by this: `KsqlScreen` is rendered
with a spread attribute in its test (a spread could override the prop), the
`BrokersScreen` tests pass helper parameters, `SubjectPane.subject` is a forward
of a prop, and `GroupScreen.groupId` is refused by the router premise's name
census in the second kui project (ADR 0324).

Fixture `imported-const-primitives`; twins `constant_file_edits_match_cold`
and `import_through_a_barrel_matches_cold`.
