# A legacy package whose `module` and `main` name different artifacts

`module` is the bundler's ESM entry; `main` is what Node's own resolver loads.
Here they are different builds, and only the ESM one can be analyzed -- the CJS
one is refused as a runtime artifact whatever it contains.

Schema v1 has no condition that distinguishes the two fields (`import` and
`require` describe a resolver choice, not these fields), and refusing the
package outright would reject every legacy dual package, including the common
case where `main` is just the CJS transpile of the same source. So the
generated contract describes the `module` build, and the review plan says so
explicitly under "legacy entrypoint resolution" -- the reviewer, not the
generator, decides whether the two builds share reactive behavior.
