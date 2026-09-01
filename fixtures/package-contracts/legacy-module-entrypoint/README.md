# Legacy module entrypoint

Pins package-contract generation for a package that publishes an exact ESM
runtime target through the legacy `module` field and has no `exports` map.
The generated callback claim must still come from the runtime artifact.

There is no `main`, no `types` and no `type` field, so the two axes diverge:
runtime resolves `legacy:module` while declarations fall through to
`legacy:index`. `legacy-module-entry` pins `legacy:module` paired with a real
`types` sibling; this fixture is the one where `module` alone is what makes the
package analyzable.
