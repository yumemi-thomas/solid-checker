# A renderer that re-exports `solid-js` is not exempt from the dependency rule

`custom-solid-renderer` publishes `createMemo` and `createSignal` straight from
its `solid-js` dependency. The dialect vocabulary knows both names, and the
bundled Solid contracts describe both -- so this is exactly the package where
the temptation is to bind the re-export by name and publish the known
semantics under this package's identity.

It is refused instead. Knowing what `solid-js`'s `createMemo` does is not the
same fact as knowing that *this installed artifact's* `createMemo` is that one;
only an accepted contract for the dependency, bound to its exact module
identity, establishes that. Name-based trust here would let any package
republish Solid's semantics by spelling.

`external-reexport` pins the same refusal for an ordinary dependency. The
`node_modules/solid-js` stub is 1.x, so this fixture also runs the v1 catalog;
it is the fixture whose missing `.gitignore` exception motivated coverage's
`checkDialectStubs` guard.
