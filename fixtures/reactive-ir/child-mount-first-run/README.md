# child-mount-first-run

**Claim (ADR 0507).** ADRs 0503-0505 prove a first run under a module-scope `render` of the component itself. A component a parent renders is mounted the same way when the parent is: the tag is self-closing with no attribute, in the parent's one unconditional return, reached through intrinsic elements only, and the parent's body is one `return <tree>` and runs nothing else (no call, member read, nested function, fragment, `throw`, class; static JSX besides the tag). The compiler evaluates such a tag while the parent builds its tree, so the child's body runs inside the parent's, outside every flush. Parents chain up to four deep.

Violations: `PlainParent` (SC2001, a render-effect first-run write), `Grand` (SC5001, two parents deep), `variants/effect-child` (SC5001, a user effect, ADR 0505).

ADR 0508: a holder may also be the dialect's `<Show when={true}>` (its value memo computes when `Show` is created and renders the child) or a `<For each={[literal, …]}>` whose one child is a parameterless arrow returning the tag (its `mapArray` runs on the parent's insert's first pass): violations `ShowParent`, `ForParent`.

Uncertifiable, one premise each: `CallingParent` (the parent runs code), `PropParent` (a prop on the tag), `ConditionalParent` (two returns), `ShowBindingParent` (`when` not the literal `true`), `ForEmptyParent` (no row), `ForItemParent` (the row takes the item).

Stubs: `node_modules` and `jsx.d.ts` copied from `effect-apply-first-run` (rc.17 published declarations); the variant shares them.
