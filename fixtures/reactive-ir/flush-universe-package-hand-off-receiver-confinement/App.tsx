import { flush } from "solid-js";
import { runStep } from "steplib";
import * as exported from "./exported-holder";

// The review of #232 (B1, B2, B5): a function stored under a key that a receiver reads, in code a package may run, is handed unless the holder provably cannot be a receiver.
// The strict prefilter (`holder_confined`) skips a plain `const` literal whose every use keeps it plain; what a use gives out is a name for the holder's own objects, and a
// key is the property key the runtime gives it, not its text. `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed as an action's step.
// The program names no action, effect or leaf primitive, so the census says the three flush codes cannot arise; a flush the package may run keeps its three uncertifiable
// rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. Every case has a key of its own, and no
// use of a receiver in the program reads a key through a value, so each row is the case's own.

// ---- B1: a rest copy holds the same functions under the same keys; given a prototype it reads the getter that runs them
const restCopyBase = {
  get g(): number {
    void runStep(() => {
      (this as any).restCopyM();
    });
    return 1;
  },
};
const restCopyHolder = {
  restCopyM(): void {
    flush();
  },
};
const { ...restCopyCopy }: any = restCopyHolder;
restCopyCopy.__proto__ = restCopyBase;
setTimeout(() => {
  void restCopyCopy.g;
});

// ---- B1: the nested literal of a holder, bound by a pattern
const nestedPatternBase = {
  get g(): number {
    void runStep(() => {
      (this as any).nestedPatternM();
    });
    return 1;
  },
};
const nestedPatternOuter = {
  nestedPatternSub: {
    nestedPatternM(): void {
      flush();
    },
  },
};
const { nestedPatternSub: nestedPatternInner }: any = nestedPatternOuter;
nestedPatternInner.__proto__ = nestedPatternBase;
setTimeout(() => {
  void nestedPatternInner.g;
});

// ---- B1: the nested literal, through a member read
const nestedReadBase = {
  get g(): number {
    void runStep(() => {
      (this as any).nestedReadM();
    });
    return 1;
  },
};
const nestedReadOuter = {
  nestedReadSub: {
    nestedReadM(): void {
      flush();
    },
  },
};
const nestedReadInner: any = nestedReadOuter.nestedReadSub;
nestedReadInner.__proto__ = nestedReadBase;
setTimeout(() => {
  void nestedReadInner.g;
});

// ---- B1: the nested literal given the prototype through a chain of members
const nestedChainBase = {
  get g(): number {
    void runStep(() => {
      (this as any).nestedChainM();
    });
    return 1;
  },
};
const nestedChainOuter: any = {
  nestedChainSub: {
    nestedChainM(): void {
      flush();
    },
  },
};
nestedChainOuter.nestedChainSub.__proto__ = nestedChainBase;
setTimeout(() => {
  void nestedChainOuter.nestedChainSub.g;
});

// ---- B1: the nested literal in an argument of a function the build reads
const nestedArgBase = {
  get g(): number {
    void runStep(() => {
      (this as any).nestedArgM();
    });
    return 1;
  },
};
function nestedArgGive(given: any) {
  given.nestedArgSub.__proto__ = nestedArgBase;
  setTimeout(() => {
    void given.nestedArgSub.g;
  });
}
nestedArgGive({
  nestedArgSub: {
    nestedArgM(): void {
      flush();
    },
  },
});

// ---- B1: a rest copy that carries a nested holder; the getter runs a method of it
const restNestedBase = {
  get g(): number {
    void runStep(() => {
      (this as any).restNestedSub.restNestedM();
    });
    return 1;
  },
};
const restNestedOuter = {
  restNestedSub: {
    restNestedM(): void {
      flush();
    },
  },
};
const { ...restNestedCopy }: any = restNestedOuter;
restNestedCopy.__proto__ = restNestedBase;
setTimeout(() => {
  void restNestedCopy.g;
});

// ---- B1: a spread copy shares the nested literal
const nestedSpreadBase = {
  get g(): number {
    void runStep(() => {
      (this as any).nestedSpreadM();
    });
    return 1;
  },
};
const nestedSpreadOuter = {
  nestedSpreadSub: {
    nestedSpreadM(): void {
      flush();
    },
  },
};
const nestedSpreadCopy: any = { ...nestedSpreadOuter };
nestedSpreadCopy.nestedSpreadSub.__proto__ = nestedSpreadBase;
setTimeout(() => {
  void nestedSpreadCopy.nestedSpreadSub.g;
});

// ---- B1: an exported holder, picked from its namespace by a key the analysis does not read (the flush is in exported-holder.ts)
const exportedBase = {
  get g(): number {
    void runStep(() => {
      (this as any).exportedM();
    });
    return 1;
  },
};
const exportedName: string = "exportedHolder";
const exportedPicked = (exported as any)[exportedName];
exportedPicked.__proto__ = exportedBase;
setTimeout(() => {
  void exportedPicked.g;
});

// ---- B2: a key spelled with a unicode escape is the key it decodes to (`\u006b` is `k`): the store, the read and a literal's method
const keyEscapeStore: any = {
  get g(): number {
    void runStep(() => {
      this.keyEscapeStoreM();
    });
    return 1;
  },
};
keyEscapeStore.\u006beyEscapeStoreM = function () {
  flush();
};
setTimeout(() => {
  void keyEscapeStore.g;
});

const keyEscapeRead: any = {
  get g(): number {
    void runStep(() => {
      this.\u006beyEscapeReadM();
    });
    return 1;
  },
};
keyEscapeRead.keyEscapeReadM = function () {
  flush();
};
setTimeout(() => {
  void keyEscapeRead.g;
});

const keyEscapeMethodBase = {
  get g(): number {
    void runStep(() => {
      (this as any).keyEscapeMethodM();
    });
    return 1;
  },
};
const keyEscapeMethod = {
  __proto__: keyEscapeMethodBase,
  \u006beyEscapeMethodM(): void {
    flush();
  },
};
setTimeout(() => {
  void (keyEscapeMethod as any).g;
});

// ---- B2: `__proto__` spelled with an escape is the prototype write
const keyProtoEscapeBase = {
  get g(): number {
    void runStep(() => {
      (this as any).keyProtoEscapeM();
    });
    return 1;
  },
};
const keyProtoEscapeHolder: any = {
  keyProtoEscapeM(): void {
    flush();
  },
};
keyProtoEscapeHolder.__\u0070roto__ = keyProtoEscapeBase;
setTimeout(() => {
  void keyProtoEscapeHolder.g;
});

// ---- B2: a number written as a key is its whole-number key (`0x1`, `1.0` and `1` are one property, read as `this[1]` and `this["1"]`)
const keyNumberBase = {
  get g(): number {
    void runStep(() => {
      (this as any)[1]();
      (this as any)["2"]();
    });
    return 1;
  },
};
const keyHex = {
  __proto__: keyNumberBase,
  0x1(): void {
    flush();
  },
};
const keyFloat = {
  __proto__: keyNumberBase,
  2.0(): void {
    flush();
  },
};
setTimeout(() => {
  void (keyHex as any).g;
  void (keyFloat as any).g;
});

// ---- B5: a name an object pattern assignment rewrites is no unique symbol
let symbolRewritten: any = Symbol();
({ symbolRewritten } = { symbolRewritten: "symbolRewriteM" });
const symbolRewrite: any = {
  get g(): number {
    void runStep(() => {
      this.symbolRewriteM();
    });
    return 1;
  },
};
symbolRewrite[symbolRewritten] = function () {
  flush();
};
setTimeout(() => {
  void symbolRewrite.g;
});

// ---- twins, silent. Each reads a key that a function stored in it is read under, and no use of the holder gives it a prototype, a receiver or a name nothing follows.
// a rest copy used as a value nothing takes
const twinRestHolder = {
  restCopyM(): void {
    flush();
  },
};
const { ...twinRestCopy }: any = twinRestHolder;
typeof twinRestCopy;

// a nested literal bound by a pattern, a member read, a chain and a spread copy, each used as a value nothing takes
const twinNestedOuter = {
  nestedPatternSub: {
    nestedPatternM(): void {
      flush();
    },
  },
  nestedReadSub: {
    nestedReadM(): void {
      flush();
    },
  },
  nestedChainSub: {
    nestedChainM(): void {
      flush();
    },
  },
};
const { nestedPatternSub: twinNestedPattern } = twinNestedOuter;
typeof twinNestedPattern;
const twinNestedRead = twinNestedOuter.nestedReadSub;
typeof twinNestedRead;
typeof twinNestedOuter.nestedChainSub;
const twinNestedCopy = { ...twinNestedOuter };
typeof twinNestedCopy;

// a pattern that names only keys no object is held under
function twinLeaf({ twinLeafCount }: { twinLeafCount: number }): number {
  return twinLeafCount;
}
const twinLeafHolder = {
  twinLeafCount: 1,
  restCopyM(): void {
    flush();
  },
};
twinLeaf(twinLeafHolder);

// a key spelled with an escape that no receiver reads, a number no receiver reads, and a symbol that is not the one the read names
const twinKeyEscape: any = {};
twinKeyEscape.\u0074winKeyEscapeM = function () {
  flush();
};
const twinKeyNumber = {
  __proto__: keyNumberBase,
  0x3(): void {
    flush();
  },
};
setTimeout(() => {
  void (twinKeyNumber as any).g;
});
const twinSymbolFirst = Symbol();
const twinSymbolSecond = Symbol();
const twinSymbol: any = {
  get g(): number {
    void runStep(() => {
      this[twinSymbolSecond]();
    });
    return 1;
  },
};
twinSymbol[twinSymbolFirst] = function () {
  flush();
};
setTimeout(() => {
  void twinSymbol.g;
});

// `Object.prototype` read, not written (a built-in method read through it, a comparison with it): no accessor is installed, so every twin above stays confined; a program that
// gave `Object.prototype` a member would ask all of them (`flush-universe-package-hand-off-object-prototype`)
const twinHasOwn = Object.prototype.hasOwnProperty;
void twinHasOwn;
void (Object.getPrototypeOf({}) === Object.prototype);

// a package takes the step, and no receiver is read
void runStep(() => {});

export function App() {
  return <p>x</p>;
}
