# valid-jsx-nesting

`SC8020` · **error** · violation

Reports intrinsic JSX nesting for which the HTML parser changes the authored
tree. These changes can make server-rendered markup hydrate against a different
browser DOM.

The rule focuses on parser behavior, including implicit paragraph/list-item
closing, nested interactive scope elements, and the required table/select
structure. It deliberately does not try to enforce every HTML content-model
restriction when the parser preserves the tree.

```tsx
// Invalid: the parser closes the outer <p>.
<p><div /></p>

// Invalid: the parser inserts <tbody>.
<table><tr><td /></tr></table>

// Valid.
<table><tbody><tr><td /></tr></tbody></table>
```

## Scope boundaries

The WHATWG in-body rules never close an ancestor unconditionally — every
implicit-close walks the stack of open elements and stops at a scope
boundary. The rule applies the same boundaries, so these common patterns are
**not** reported:

```tsx
// Valid: the inner <ul> is a "special" element and terminates the li
// implied-end-tag walk — nested lists are preserved verbatim.
<ul><li>outer<ul><li>inner</li></ul></li></ul>

// Valid: same for <dl> around dd/dt.
<dl><dd>outer<dl><dd>inner</dd></dl></dd></dl>

// Valid: <button> is a button-scope boundary, so the <div> does not close
// the paragraph (p is not "in button scope").
<p><button><div /></button></p>

// Valid: <td> is a default-scope boundary, preserving the outer button.
<button><table><tbody><tr><td><button /></td></tr></tbody></table></button>
```

While these are:

```tsx
<ul><li>first<li>second</li></li></ul>     // Invalid: li closes li (no list between).
<button><span><button /></span></button>  // Invalid: button in button scope.
<a href="#o"><span><a href="#i" /></span></a> // Invalid: a reopens/splits a.
```

Concretely, per the spec: `li` (and `dd`/`dt`) walks stop at any *special*
element other than `address`, `div`, or `p`; the `p`-closing start tags
(`div`, `ul`, `p`, headings, `search`, …) close a paragraph only when it is
in *button scope* (boundary list: `applet`, `caption`, `html`, `table`,
`td`, `th`, `marquee`, `object`, `template`, plus `button` itself and the
SVG/MathML integration points); `button`, `a`, and `nobr` use the default
scope list (the same minus `button`). `form`-in-`form` follows the form
element pointer, which ignores intervening elements except `template`.

Component boundaries are opaque. The checker does not infer what DOM a child
component returns, so `<p><Child /></p>` is not guessed to be invalid.

## Known boundaries

Two parser behaviors are deliberately not modeled; both err toward silence,
matching the rule's only-when-the-parser-changes-the-tree policy:

- **SVG breakout:** HTML-only tags inside `<svg>` (outside `foreignObject`)
  make the parser close the foreign content and hoist the element out. The
  rule skips subtrees under `<svg>` entirely.
- **`<option>`/`<select>` text-only content:** the parser drops or moves
  some non-text children of `option` and unusual `select` children beyond
  the structural set already checked.

See the WHATWG HTML parsing rules for [parsing `in body`](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody),
[the scope definitions](https://html.spec.whatwg.org/multipage/parsing.html#has-an-element-in-scope),
the [*special* category](https://html.spec.whatwg.org/multipage/parsing.html#special),
and [parsing tables](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable).
