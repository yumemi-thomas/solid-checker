# A shorthand property is resolved by scope, never by spelling

`{ tracked }` writes one identifier where a key and a value both stand, and a
symbol query at that span answers with the *property's* symbol. The value
binding is named only by the binder's resolution of that reference, which is
scope-exact. Every export here pairs a shorthand with a same-spelled
declaration the shorthand cannot see.

Proved (the returned object carries the accessor the shorthand actually names):

- `scopedShorthand` -- a sibling block declares a decoy accessor of the same
  name; the visible declaration wins and the decoy neither is chosen nor makes
  the choice ambiguous;
- `writtenShorthand` -- the same object written longhand, so the ordinary value
  path and the shorthand path have to agree;
- `importedAccessorShorthand`, `defaultReexportShorthand`,
  `namedReexportShorthand`, `exportAllShorthand` -- the accessor is declared in
  another module and reached through a named import, a default re-export, a
  renamed re-export chain, and an `export *`. The join follows the specifier to
  the exporting declaration and matches it exactly.

Unproved (no accessor is in scope at the shorthand, so no claim may be made):

- `unprovenShorthand` -- the only accessor of that spelling is block-scoped and
  out of scope; the shorthand names the module-scope plain function;
- `shadowedShorthand` -- a parameter shadows nothing reactive;
- `importedShorthand` -- resolves to an import specifier declaring no accessor;
- `namespaceShorthand` -- a namespace object.

`ambiguous.ts` / `ambiguous/index.ts` (a name two relative targets could
answer), `cycle-a.ts` / `cycle-b.ts` (a re-export cycle) and
`node_modules/bare-package` are *not* reached from `./index.ts` and so are not
part of the published closure this contract describes. They are retained from
the fixture's earlier life as a whole-project fixture; the resolver claims they
were written for are not asserted here.
