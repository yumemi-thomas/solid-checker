# leaf-scope-exact-method

**Claim (ADR 0209).** A method called in a leaf owner's callback on an object whose class is exactly known is followed into its body, like a helper called there. The object is a `const` bound directly to `new C(…)`, and the method is declared in `C`. Inside that method, `this.m()` is followed too (`cleanup::exact_instance_method`).

| Case | Finding | Why |
| --- | --- | --- |
| `ExactInstance` | none | `Camera.perspective` and its `this.update()` create nothing in the leaf scope |
| `ExactInstanceForbidden` | `leaf-owner-forbidden-call` violation | `Leaky.start` registers a cleanup in the leaf scope |
| `SuppliedInstance` | `SC9012` uncertifiable | a caller-supplied object may be another one |
| `WidenedInstance` | `SC9012` uncertifiable | the annotation resolves `Base.run`, while `Derived.run` runs |
| `ReassignableInstance` | `SC9012` uncertifiable | a `let` may hold another object |

`solid-js.d.ts` is copied from `leaf-scope-builtin-method`.
