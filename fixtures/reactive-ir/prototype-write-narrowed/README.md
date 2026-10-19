# prototype-write-narrowed

**Claim (ADR 0314).** A write through a prototype under a key no proof names (`R.prototype[key] = ...`) vetoes the exact-member proofs of ADR 0209/0211 only for objects whose prototype chain holds that prototype: `R` a class/function declared in the writing file and never rebound, or a parameter of a static method called only through the class's own name with a global only TypeScript's default libraries declare. Unaffected cases stay clean; written classes, bases and unresolved `extends` are `SC9012`. Refusals: `prototype-write-object-host`, `-escaped-helper`.
