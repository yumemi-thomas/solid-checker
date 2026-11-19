# action-in-owned-scope-prototype-sites

**Claim (ADR 1114 family 3: the site half of ADR 1108 part 2).** Where a package the program loads writes a member on a built-in's prototype by name and may reach a Solid action, every call that may run that name is an uncertifiable site (`package-may-call-action:reaches-solid`, provider `prototype-write:<name>:<root>`), not only a direct call of a method the standard library declares. Never a violation.

- **Uncertifiable**: `CalledThroughCall` (`a.reverse.call(a)`), `CalledThroughApply`, `CalledThroughReflect` (`Reflect.apply(a.reverse, …)`; the member handed to it is a read row too), `CalledDestructured` (`const { reverse } = a; reverse()`: the pattern's read and the call), `CalledOnInterface` (a receiver typed by a project interface), `CalledOnAny`, `BareInherited` (`dispatchEvent(…)`: the window inherits `EventTarget.prototype.dispatchEvent`), and the library's implicit calls `TestRunsExec` (`/x/.test` runs `exec`), `MapRunsSet` (`new Map([...])` runs `set`), `ToStringRunsJoin`, `TemplateRunsJoin` (a template's substitution is converted to a string, which runs an array's `join`; the site is the substitution, where V8 places it), `StringifyRunsToJson` (`Date.prototype.toJSON`), and `AccessorRead` (`void dv.byteLength`: `fz-psites` defines a getter on `DataView.prototype`).
- **Uncertifiable, unchanged (part 1)**: `CalledDirectly` (`new Float32Array(1).reverse()`).
- **Silent**: `WrittenByClearPackage` (`fz-psitesclear` writes `Uint16Array.prototype.fill` and names no Solid package), `Unwritten` (`map`, `trim`, `slice`: no package writes them), `CalledInHandler`.

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-prototype-sites`): on main (e637f50f2) 13 unproven-clean sites; here every site covered-uncertifiable, the silent components quiet.

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js`, `@solidjs/*`, `solid-js.d.ts` and `.solid-checker/runtime.json` (closed boundary) are those of `action-in-owned-scope-prototype-writes`.
