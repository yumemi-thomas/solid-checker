# leaf-scope-exact-method

**Claim (ADR 0209).** A method called in a leaf owner's callback on an object whose class is exactly known (a `const` bound directly to `new C(...)`, method declared in `C`, `this.m()` followed too; `cleanup::exact_instance_method`) is followed into its body like a helper: `ExactInstance` none, `ExactInstanceForbidden` a `leaf-owner-forbidden-call` violation; supplied, annotation-widened and `let` instances are `SC9012`. `solid-js.d.ts` is copied from `leaf-scope-builtin-method`.
