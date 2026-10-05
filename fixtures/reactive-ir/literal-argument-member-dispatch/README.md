# Primitive receivers select built-in members

A helper that invokes a member of its parameter (`key.replace(...)`) leaves
the choice of implementation to each call site. When the argument there is a
literal (string, number, boolean, bigint, template or RegExp literal), it is a
fresh value whose members all come from its built-in prototype, and no
built-in reads reactive state, so the call site carries no dispatch
obligation. The same holds when Type Facts resolves the helper's member call to
a primitive wrapper's standard-library method (`String.toUpperCase`), whatever
argument arrives (ADR 0190).

- `StringLiteral`, `TemplateLiteral`: clean (previously `SC9012`
  uncertifiable).
- `TypedString`: clean; `shout`'s `message.toUpperCase()` is
  `String.toUpperCase`.
- `ChainedBuiltin`: clean; `message.trim().split(" ")` takes no argument at
  `trim`, so its resolved call is demanded because its root is a parameter.
- `ObjectBuiltin`: `Date.getTime` is an object type's method, which a
  subclass can override, so `SC9012` stays.
- `ObjectArgument`: resolved through the object's own member, unchanged.
- `UnknownArgument`: a caller-supplied value with no selectable
  implementation stays `SC9012` uncertifiable.
- `LiteralWithReadingReplacer`: the literal does not settle what the callee's
  own replacer callback reads. `fill` reads `name()` inside the
  `String.prototype.replace` replacer, synchronously, during the call.

The stub `solid-js.d.ts` declares only `createSignal`, whose tuple shape is
the published one; no finding here depends on another signature.
