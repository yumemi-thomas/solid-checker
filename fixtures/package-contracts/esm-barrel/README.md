# A `.mjs` barrel described by a `.d.mts` sibling

The runtime artifact is `index.mjs` and the declarations are `index.d.mts` --
the explicit-extension pair, not the `.js`/`.d.ts` one every other barrel
fixture uses. The contract has to record the two different paths on one case.

The barrel's export list is deliberately every way a name can arrive at the
public surface without a function declaration behind it:

- `createValue` is re-exported from a sibling module;
- `createAlias` is a local binding to that same imported function;
- `createLocal` is a parenthesized arrow;
- `createConditional` is a conditional expression over two named function
  expressions;
- `createFromMemberFactory` is the result of a method call on an object
  literal, and `factoryComponent` the result of the same call through a
  `Proxy`, so no function body is summarized for either;
- `bootstrapSource` is a string that happens to spell a function. It is the
  negative control and must stay `plain`.

The claim is that the six callables are callable and the string is not --
established from the declarations where no body was read, never from the
spelling of the initializer.
