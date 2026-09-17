# Default declaration export census

The CLI artifact census now distinguishes a default declaration's local name
from its public export name. `export default function Local() {}` and
`export default class Local {}` expose `default`; they do not also expose
`Local`. A separate `export { Local }` continues to publish the named binding.

This follows the [ECMAScript export-entry rules](https://tc39.es/ecma262/2025/multipage/ecmascript-language-scripts-and-modules.html).
Native authenticated archive replay already used those semantics. The bug was
in the CLI description walker, which continued from the default branch into
ordinary named-declaration handling and supplied a nonexistent public name.
The correction applies to runtime and declaration censuses without relaxing
native equality, export binding, dependency or semantic proof checks.

The motivating refusals were `solidPlugin` in Vite Plugin's declaration census
and `Article` in Solidbase's. The scoped probes now certify Vite Plugin's root
in both Solid 2 contexts and 33 Solidbase entrypoints. They remain partial rows;
this is not a coverage-denominator correction. Exact importer, artifact,
conditions, trust and receipt verification remain unchanged.

Focused tests cover functions/classes with and without explicit additional
named exports. No checker diagnostic, shared protocol or receipt format changes.
