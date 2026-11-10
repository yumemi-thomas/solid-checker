# ref-await-in-place-layers

ADR 0938: an in-place layer (IIFE, inline `forEach`, a named function called once) runs where its call is written. In a `ref` callback before any await: SC1001 V. Past an await on every path: silent (Chrome silent, witness landed); past a skippable one: U. `BodyAsyncHelper` stays the summary's U.
