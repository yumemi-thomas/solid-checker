# v1/jsx-no-duplicate-props

`SC8003` · **error** · violation

The same effective JSX property is written more than once — where "same" means
the occurrences land in one single-winner slot after the Solid 1.x compiler
lowers them — or an element uses multiple competing sources of child content.

## What it does

Compares direct attributes with properties from inline object-literal spreads.
`attr:`/`prop:` aliases share the plain spelling's slot, so an alias cannot
evade the check. It also rejects combinations of JSX children, a `children`
prop, `innerHTML`, and `textContent`, because each writes the element's
content.

### Intrinsic elements: the compiler's slot model

Event-shaped names on an **intrinsic element** — the tags the compiler lowers
to DOM operations — follow the compiler's actual lowering
(`babel-plugin-jsx-dom-expressions@0.40.7`), not a name comparison. Two
occurrences are a duplicate only when both land in the same single-winner
slot:

- a **delegated** event (`click`, `input`, `keydown`, …) written in the plain
  `on*` form lowers to the property write `el.$$event = handler` — a later
  occurrence silently overwrites an earlier one;
- a **statically string/number-valued or bare** `on*` prop never becomes a
  listener: it is frozen into the template, where the HTML parser keeps the
  *first* occurrence of an attribute name (this slot is shared with the
  `attr:` spelling of the same name).

Everything else attaches and stays live, so it is **not** a duplicate:
`on:evt` binds a bubble listener and `oncapture:evt` a capture listener via
separate `addEventListener` calls; a non-delegated plain `on*` event attaches
one listener per occurrence; a delegated `onClick` and an `on:click` on the
same element both fire. eslint-plugin-solid folds all of these onto one
lowercase name and reports them — runtime-legal code — so this is a
deliberate, compiler-evidenced divergence from upstream (no upstream corpus
case pins the folding; the parity ledger is unaffected).

The static-value half of the model is a *node-kind* test, matching the
compiler's inline branch: `Expression::StringLiteral` and
`Expression::NumericLiteral` freeze into the template, everything else lowers
at runtime. So `{0x10}` and `{1_000}` are frozen (the compiler inlines their
numeric value), while `{-1}`, `{+1}`, `{NaN}`, and `{Infinity}` are unary
expressions and identifiers that are not — a distinction a "does this text
parse as a number" test gets wrong in both directions.

### Components: key identity

A **component** tag never reaches that lowering. Its props are collected into
a plain object, so the only slot is the key exactly as written, and a repeated
key is a plain later-wins overwrite:

- `<MyComp onSave={a} onSave={b} />` and `<MyComp on:click={a} on:click={b} />`
  are duplicates — the DOM slot model would wrongly excuse both;
- `<MyComp onClick={a} onclick={b} />` and `<MyComp attr:title={a} title={b} />`
  are *not* — those are distinct object keys, with no DOM aliasing to merge
  them.

(A namespaced prop on a component is separately reported by
[no-unknown-namespaces](no-unknown-namespaces.md), which is about the prefix
having no effect there at all.)

## Why is this bad?

For colliding slots, only one write survives — a later `$$event` assignment,
or the first parsed template attribute — so the other occurrence is dead, and
changing attribute order or spread contents silently changes which value
reaches the DOM. Duplicate `class` props are especially fragile; independent
conditional classes belong in `classList`.

## Examples

Incorrect:

```tsx
<button class="base" {...{ class: active() ? "active" : "" }} />
<button onClick={save} onClick={audit} />   {/* delegated: only audit runs */}
<MyComp onSave={save} onSave={audit} />     {/* one props key: audit wins */}
<div innerHTML={markup}>{fallback}</div>
```

Correct:

```tsx
<button class="base" classList={{ active: active() }} />
<button on:click={save} oncapture:click={audit} />  {/* both listeners fire */}
<button onClick={save} on:click={audit} />          {/* both listeners fire */}
<MyComp onClick={save} onclick={audit} />           {/* two distinct keys */}
<div>{fallback}</div>
```

## How to fix

Remove the dead occurrence or combine the values into one prop. Choose exactly
one mechanism for element content. No automatic fix is offered because deciding
which value should survive is a semantic choice.

## Known limits

The slot model is static: a spread whose object is not an inline literal is
invisible here (as upstream), and the static-value test recognizes only
literal strings and numeric-literal lexemes, exactly the shapes the compiler
freezes into the template. A constant that *folds* to a string or number is
not one of them, matching the compiler, which does no folding either.

## Related

- [event-handlers](event-handlers.md) — statically-valued `on*` props
- [prefer-classlist](prefer-classlist.md) — conditional class composition
- [no-innerhtml](no-innerhtml.md) — unsafe markup injection
