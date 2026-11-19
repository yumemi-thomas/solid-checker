# after-await-operators

**Claim (ADR 1113, narrowed).** An operator an async memo runs after its `await` may run project code with no call written there: a coercion reaches `toString`, `valueOf` or `Symbol.toPrimitive`; `in` reaches `ToPropertyKey` and a proxy's `has` trap; `instanceof` reaches `Symbol.hasInstance` and, with an object on the left, a proxy's `getPrototypeOf` trap; `delete` reaches a proxy's `deleteProperty` trap. Each such operator is a site (SC1002 uncertifiable, `read-after-await-unaccounted`) unless it runs nothing:

| case | row |
|---|---|
| `` `${coerced}` `` (an object whose `toString` reads), `"k" in probed`, `err instanceof Error`, `delete removable.k`, `` `${mixed}` `` (`string` or an object) | a row at the operand or the operator |
| `label === "y"`, `typeof coerced`, `!probed`, `i < 2` and `i++` on a `number` counter, `total += i`, `` `${text}!` `` (`text: string`) | none |
| `handler.ts`, a promise handler in a file with no `await`: `` `${coerced}` `` | a row |
| `handler.ts`: `value - started`, two numbers | none |

A coerced operand is a primitive when Type Facts give it a demanded primitive domain with no object in it (facts schema 98 adds `relation_operators`, the operands of `in` and `instanceof`). Every `instanceof` stays a site: a primitive left operand still reads the right one's `Symbol.hasInstance`, and an object on the left reaches a `getPrototypeOf` trap whatever the right is.

Stubs: `solid-js.d.ts`, `node_modules/solid-js/package.json` and `tsconfig.json` copied from `after-await-net`.
