# dead-site-unused-class

**Claim (ADR 1021).** A read after `await` (SC1002) in a constructor, an instance field's helper, a getter or a method of a class or object literal nothing constructs or accesses never runs, so it is uncertifiable (`function-unentered`), never a violation. Every reference is inert (`void K`, `typeof K2`, `K;`, `void (o as unknown)`) or the `extends` of such a class.

Uncertifiable, each a violation on main 993e83f74: `CtorNever`, `CtorSuperNever`, `FieldHelperNever`, `GetterNever`. `MethodNever` was uncertifiable on main (`handed-read-after-await`) and stays.

Violations (the construct runs): `CtorNew`, `CtorSuperNew`, `FieldHelperNew`, `GetterRead`, `Wrapped` (the class handed to a function that constructs it), `Reflected` (`Reflect.get` reads the getter).

Chrome: `fixtures/runtime-oracle/dead-site-unused-class` (quiet) and `-live` (6 of 6 covered). Stubs: `component-mounted-in-disposal`'s. `tsc --noEmit` clean.
