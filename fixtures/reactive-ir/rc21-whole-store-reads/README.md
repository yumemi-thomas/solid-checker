# SC1001: reads of a whole store

Measured on the published rc.21 dev build in a strict-read window, one probe per
form with a try and an after-marker (ADRs 0620, 0621). `deep`, `JSON.stringify`,
`Reflect.get`, `Array.from` and a template substitution warn on any store;
`for…of`, array and call spreads and array destructuring read `length`, so they
need an array; `{...s}`, `Object.values`, `entries` and `assign` read a key the
store holds, so `{}` is silent and an unknown value is unproven. `snapshot`,
`Object.keys`, `in`, `hasOwn`, `for…in` and `!!s` read no key.
