# Namespace re-export identity

The root exports a module namespace named `Bucket`; its target also exports a
callable named `Bucket`. The root must bind the namespace's exact compiler
entity, never the member's callable identity. `./renamed` carries that same
namespace through an ordinary named re-export. Both are non-callable values.

`./named` and `./star` are controls: they really export the callable member,
including its callback invocation. The missing module behind `./unresolved`
must retain an explicit artifact refusal. These are ordinary TypeScript module
semantics; no Solid API stubs or new checker diagnostics are involved.

`./shadowed` and `./shadowed-value` place a bare star before an explicit export
of the same name. The explicit namespace and numeric binding must win; neither
may inherit the star target's callable entity. TypeScript 5.9.3 accepts both
files without diagnostics and reports zero call/construct signatures for their
`Bucket` exports. Before the precedence correction, both generated callable
proposals. The fixture now pins all six successful cases and the missing target.
