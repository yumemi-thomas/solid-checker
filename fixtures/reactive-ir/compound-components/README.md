# compound-components

A function stored in an object is a component when every way the program can obtain the member is a JSX tag (ADR 0342). `<Split.Panel>` compiles to `createComponent(Split.Panel, props)`, so with no other reference to the object or the function, `createComponent` is the only caller, exactly as for a directly tagged function. Without the census the stored reference is an escape (ADR 0331) and every body read here is uncertifiable.

The census: the holder is a `const` bound to a closed object literal (no spread, computed key, duplicate, getter, setter or method) or to the standard library's `Object.assign(target, literal...)`; every Type Facts reference to it is its declaration, its module surface or the leftmost name of a JSX tag whose keys name a member; and the function itself has no other reference than the stored value, its declaration and its surface. A member no tag renders stays unaccounted.

- `Literal.tsx` (positive): `{ Root, Panel, Handle }` rendered as `<Split.Panel/>`, `<Split.Handle></Split.Handle>` (a closing tag is the same use).
- `Assign.tsx` (positive): `Object.assign(TabsRoot, { List, Item })`; `<Tabs>` renders the target and `<Tabs.List>` a member.
- `Nested.tsx` (positive): a nested namespace (`<Card.Header.Title/>`), an anonymous arrow member (`<Card.Footer/>`) and a data member never tagged.
- `Dialog.tsx`, `index.ts`, `Screen.tsx` (positive): the holder is exported through a barrel and tagged in another module; an export is module surface, as for a plain component.
- `Helper.tsx` (positive): both members call a helper whose strict read is reported at each call written directly in a member body; a member held in an object is a render there, not a retained callable (ADR 0292's `lazy` default is the same disposition).

Negatives are in `compound-components-escapes`.
