# A legacy package whose `module` and `main` name different artifacts

`module` is the bundler's ESM entry; `main` is what Node's own resolver loads.
Here they are different builds, and only the ESM one can be analyzed -- the CJS
one is refused as a runtime artifact whatever it contains.

Schema v1 has no condition that distinguishes the two fields (`import` and
`require` describe a resolver choice, not these fields), and refusing the
package outright would reject every legacy dual package, including the common
case where `main` is just the CJS transpile of the same source. So the runtime
axis prefers the `module` target and records `legacy:module`; `legacy-module-entry`
pins that selection on a package this one deliberately is not.

This package publishes no `types` and no `typings`. The declarations axis is
not affected by `module` and falls back to `main`, so it lands on the CJS build,
which has no format-matching `.d.cts` sibling. The package is refused before the
runtime source can masquerade as its own declaration. That is the fail-closed
answer, not a placeholder: nothing in the artifact declares the ESM build's
surface, and the generator does not get to assume the two builds agree.
