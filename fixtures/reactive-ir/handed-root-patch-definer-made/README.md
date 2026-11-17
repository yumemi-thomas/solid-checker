# handed-root-patch-definer-made

ADR 1045 positive: `Number` is handed on, and the file's only reflective definers are `Object.defineProperty`,
`Reflect.defineProperty` and `Object.defineProperties` of the unshadowed globals, called on an object the file makes
(ADR 1024's `Made`: a `const` of `{}`, the literal `{}`) under keys no literal names: the shapes of
`@tanstack/solid-router` 2.0.0-rc.9 `link.jsx:317-332` and `@tanstack/solid-query` 6.0.0-rc.5 `dev.js:565`. They define
nothing on a standard root and no definer is patched, so there is no `ReflectiveDefinition(*)` and `Number.isFinite` is
certified (it was `standard-member-impostor`).
