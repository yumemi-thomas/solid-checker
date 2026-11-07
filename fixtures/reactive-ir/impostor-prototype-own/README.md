# impostor-prototype-own

ADR 0876's negative: a function's own `prototype`, immediate, in a program that writes no `prototype` key, reaches nothing standard:
`Promise.resolve().then` keeps its proof, so the write in the callback is what it is on a fresh stack: silent. A prototype only compared
(`getPrototypeOf(v) === Object.prototype` is a read, not a write) raises nothing. `array` keeps its `forEach` violation.
