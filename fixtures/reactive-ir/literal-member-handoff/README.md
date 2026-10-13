# Literal member hand-off

Claim (ADR 0286): a member read such as `tag.label()` that source discovery's
type-only fallback admits because Type Facts types the member `Accessor<T>` is
a proven SC1001 only while no code can have replaced that member. Every read of
the member's property symbol is judged by its receiver.

- Positive (`App.tsx`, SC1001): `Kept` reads through the `const` binding of the
  literal that declares the member; `MemberOnly` passes `tag.label` itself and
  reads `tag.other`, neither of which can replace `label`; `FactoryKept` and
  `ArrowKept` read an object a closed project factory in `factories.ts` returns
  fresh (through its own binding used only for that return, and as an
  expression-bodied arrow returning the literal). Since ADR 0294 these four
  are proven by what the literal stores (a `createSignal` accessor), not by
  the member's `Accessor<T>` type.
- Uncertifiable (`App.tsx`, `SC9012`): `External` reads an interface-declared
  member through a receiver that is no project literal. Its type is the only
  evidence, and ADR 0294 no longer lets a type prove a reactive read.
- Negative (`App.tsx`, silent): `relabel` (`factories.ts`) replaces `label`
  through a generic parameter, a write that names the constraint's property
  and never the literal's, so no census of the member's own symbol sees it.
  `Tagged` (shorthand) and `Longhand` (`{ label: label }`) hand the binding to
  it as an argument; `FactoryHanded` reads an object its factory handed off
  before returning it; `Consumer` hands off an object a factory returned;
  `Alias` writes through an alias with a widened annotation; `Written` writes
  through the receiver itself under a type assertion; `Method` hands `this`
  off from a method of the literal; `Declared` hands off a project literal
  whose member an interface declares.

The silent cases are silent rather than uncertifiable, as every other refusal
of the type-only fallback is (a written binding, a receiver that is the member
itself): the fallback is the only evidence the read is reactive, and withholding
it leaves no claim to obligate.

Remaining approximations (docs/precision-backlog.md): the verdict is per
property symbol, so one escaped read withholds every read of that symbol; a
hand-off to project code that provably writes nothing (a JSX prop, a reader) is
still a hand-off; an interface-declared member read through an Unknown receiver
(a parameter, a context value) keeps the type-only premise.

`solid-js.d.ts` is `context-member-identity`'s stub; its claim-bearing
signatures are copied from published 2.0.0-rc.13. The sources pass
`tsc --noEmit` (strict) against the stub and against installed rc.13 with
`skipLibCheck: false`.
