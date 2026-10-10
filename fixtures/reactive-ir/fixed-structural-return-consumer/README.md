# Fixed structural return consumer controls

ADR 0172 consumer controls over the stable tuple/object constructors. The
fixture issuer supplies exact whole-parameter carrier contracts. It does not
claim runtime census coverage of a published primitives package.

BadTuple must report its untracked reactive read; GoodObject stays clean in
JSX. Replacing a tuple member, or replacing an object member through an exact
alias, must invalidate its former reactive identity. An unknown index must
not acquire the first slot's identity.

BadObject must report the same untracked read. Rebinding a destructured member,
deleting a property, and passing a container to an unknown mutator must not
retain its reactive identity. Computed dispatch and this fixture's namespace
admission stay explicitly uncertifiable; no member is chosen by spelling.

The reduced Solid stub preserves rc.9's branded
`Refreshable<Accessor<T>>` first slot. Carrier declarations describe this
synthetic fixture's exact interface. Explicit `() => number` type arguments
make plain replacement functions valid against published typings too.

Published-typing control: TypeScript runs against the retained published
`solid-js`, `@solidjs/signals` and `@solidjs/web` 2.0.0-rc.9 artifacts, replacing
the Solid stub, with `--noEmit --jsxImportSource @solidjs/web --skipLibCheck`.
It reports no diagnostics for App.tsx. `skipLibCheck` excludes defects in the
prerelease library declaration files themselves, not this consumer's types.
