# package-write-entries

**Claim (ADR 0949).** A write a package performs on its own signal is a defect where a project's owned scope runs it, and the project answers for the entry: before ADR 0949 the package-site violation was dropped (ADR 0284) and the project call, getter, render or prop-setter hand-off that runs it got no row at all. Chrome (rc.21 development build, the runtime oracle): `REACTIVE_WRITE_IN_OWNED_SCOPE` at the project line of each positive.

`App.tsx` (package `node_modules/dep-write`, a synthetic package of this fixture):

- Obligation (SC2001, uncertifiable, `package-finding-at-entry:SC2001`): `CalledInMemo` (`createMemo(() => bump())`), `GetterInMemo` (`createMemo(() => store.bumped)`: the project reference to the holder is the entry), `CalledInBody` (`bumpNow()` in a component body), `Rendered` (`<Bump />`: the element is the entry).
- Obligation of the claiming rule from the project's side, `setter-handed-to-package-dispatch`: `PropSetter` (`<SetBy set={setN} />`: `SetBy` calls `props.set(1)`, a member of its own parameter the package cannot resolve, and the element hands the signal's setter in: the call may be that setter, which `REACTIVE_WRITE_IN_OWNED_SCOPE` rejects in the owned scope that renders the component). The package's own findings, an SC9012 dispatch obligation and an SC1001 `read-backing-unproven`, stand beside it at the element; an SC9012 withholds the site for every runtime code (`scripts/runtime-oracle.mjs` `withholdingRules`), which is no claim of SC2001's: the row is. `package_entry_table` asks it where the function holds an `exported-parameter-member-dispatch` or `handed-member-dispatch` obligation and the render or call hands an exact setter (`source_discovery.setters`) as an attribute's expression or an argument. A setter inside an object literal argument (`Bump({ set: setN })`) is not read yet (docs/open-work.md); the SC9012 obligations stand there.
- Obligation of other rules beside SC2001: `GetterInMemo` also holds an SC9012 and an SC1001 from the getter's dropped findings.
- Silent: `Handler` (`reset()` in an event handler writes legally; the package finds nothing in `reset`, so there is no finding to hand over).

The build before ADR 0949 certified the whole project clean (0 rows) while Chrome fired the code at the four SC2001 sites and at `PropSetter`.

Never a violation from this path (the package's own execution is not proven by a path the call walk does not follow), never silence: `package_drop::hand_off` is the one place a package finding leaves the report and it stands again, as an uncertifiable finding of the same rule, at each project site that enters the function holding it.
