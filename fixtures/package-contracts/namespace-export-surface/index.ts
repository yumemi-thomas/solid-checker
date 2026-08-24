// A plain exported namespace. `Config` is an object at runtime; `Config.inner`,
// `Config.helper` and `Config.Nested` are its members, and none of them is
// importable from this module.
export namespace Config {
  export const inner = 1;

  export function helper(): number {
    return inner;
  }

  export namespace Nested {
    export const deep = 2;
  }
}

// A merged declaration: the class is the runtime value the name binds, and the
// namespace body hangs a static member off it. `Merged.marker` resolves;
// `import { marker } from "namespace-export-surface"` does not.
export class Merged {}

export namespace Merged {
  export const marker = 1;
}

// Not exported at all. Its own `export const` is a member of the namespace
// object, so it is not part of this module's surface either.
namespace Unexported {
  export const hidden = 3;
}

// The negative controls: real module-level exports that must survive.
export const settings = { retries: Unexported.hidden };

export function plainFunction(value: number): number {
  return value;
}

// M1 regression: a module-level export whose name is shadowed by a nested
// namespace member of the same name. `exports` is sorted by span, and
// `internal`'s `export function helper` sits earlier in the source than the
// module-level `export const helper`, so a name-keyed enumeration that walks
// every `ExportFact` (nested ones included) can bind `helper` to the wrong
// specifier's type facts -- the function's, not the number's. `helper` here
// is a number; nothing may publish it as `kind: "function"`.
namespace internal {
  export function helper(v: number): number {
    return v;
  }
}
export const helper = internal.helper(41);

// M2 regression: a class static block is neither a function body nor a module
// block, so `exported_bindings` used to admit a declarator inside one onto
// the export surface. `insideStaticBlock` is not reachable from outside
// `Holder` and must not appear; `Holder` itself is a real export and must keep
// `kind: "function"`.
export class Holder {
  static {
    const insideStaticBlock = 1;
    void insideStaticBlock;
  }
}

// M2 regression, class-expression form: `boxed`'s own declarator span
// *contains* the class expression (it is the initializer), the reverse
// containment from the static block's declarator inside the class body. Both
// directions must be decided correctly: `boxed` survives as a real export,
// `hiddenB` does not.
export const boxed = class {
  static {
    const hiddenB = 2;
    void hiddenB;
  }
};
