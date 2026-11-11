# package-handed-values

**Claim (ADR 0949, generalized claims).** A package analyzed as source is entered by a project call, `new`, render or tagged template, or run by a project value that carries it. The call walk reads what a call hands only where it resolves the callee to a function and follows the argument; a constructor, an object's getter, a hop through a primitive, a carrier, `Reflect.apply`, a JSX prop and a value inside a larger one are not followed. A capability the project hands in by such a path (a signal's setter, a reactive accessor, a store, an action) may be used by the package, and where it is used in an owned scope Solid's dev build throws or logs its code. Before this the program was certified clean at each site (SC9012 `value-*` rows at most, a withholding the runtime oracle credits and the coordinator's ruling does not count). W's package-site cells (ADR 0957) cross these entries with each capability; Chrome fires the runtime code at each and the cells were false cleans on 833a785f2.

`App.tsx` (package `node_modules/dep-handed`, a synthetic package of this fixture):

- Uncertifiable row of the claiming rule, `setter-handed-to-package` (SC2001): `Constructed` (`new Holder(setN)`), `Reflected` (`Reflect.apply(plain, undefined, [setN])`), `Hopped` (`createComponent(Use, { set: setN })`), `Carried` (`<Lazy set={setN} />`, `Lazy` = `dynamic(() => Use)`: a carrier, whose tag is an entry), `Held` (`make(setN).value`: the package's getter captures the parameter), `Shorthand` (`Use({ set })`: a shorthand property resolves through its binding).
- `store-handed-to-package` (SC2003 and SC1001): `Store` (`<Use store={state} />`). `accessor-handed-to-package` (SC1001): `Accessor`. `action-handed-to-package` (SC2002): `Action`.
- The package's own findings (SC9012, SC1001, SC2003 `package-finding-at-entry`) stand beside them at the entries; at the carrier's reference in `dynamic(() => Use)` and at its tag.
- The call walk's own: `Walked` (`plain(setN)`): the parameter of an exact call is captured by no nested function, so the walk decides: an SC2001 violation, and no claim beside it.
- Silent: `Plain` (`<Label />`: nothing handed).

Not claimed (docs/open-work.md): children (`<Pkg>{setN}</Pkg>`); an async accessor's SC5001 (the dialect emits that rule only through the async-read channel); a callee that resolves to no function and names no package symbol.
