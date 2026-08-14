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

Component boundaries are opaque. The checker does not infer what DOM a child
component returns, so `<p><Child /></p>` is not guessed to be invalid.

See the WHATWG HTML parsing rules for [parsing `in body`](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-inbody)
and [parsing tables](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intable).
