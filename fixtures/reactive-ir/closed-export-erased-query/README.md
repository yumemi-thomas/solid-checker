# A type query is not a runtime entry

`helpers.ts` exports three helpers that call `.label()` on their parameter.
The project is an application (`"private": true`), so ADR 0193 closes the
program, and ADR 0203 drops an export's caller-supplied-member obligation when
every entry is a call the graph resolves.

- `describe` is also named by `typeof describe` inside two type aliases. A
  TypeScript type query is erased, so those references enter nothing: the
  declaration obligation clears. The reactive `label` passed at the `App` call
  is still reported there (`SC1001`, read through `describe`).
- `observed` is named by a runtime `typeof observed`, a value expression. It
  is not a type query, so the obligation stays (`SC9012`).
- `escaped` is put in an exported array, which hands the function out. Its
  obligation stays (`SC9012`).

Type-checks clean against the published `solid-js@2.0.0-rc.13` declarations
as well as against the local stub.
