# A prototype proved plain installs nothing

ADR 0643: `null`, a literal of functions and primitives, a `.prototype`, `Object.create(null)`. A computed `["__proto__"]` key and a
shorthand define an own property: the Get finds nothing, and Chrome is silent.
