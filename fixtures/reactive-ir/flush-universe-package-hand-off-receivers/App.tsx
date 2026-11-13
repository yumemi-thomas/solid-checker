import { flush } from "solid-js";
import { runStep } from "steplib";
import { importedReader } from "./foreign";

// The check of #227 (G1) and its neighbours: an object's members are read through `this` and `super`, which no reference of the object shows. `steplib` (its source is not
// read: its manifest names no Solid package) runs what it is handed as an action's step. The program names no action, effect or leaf primitive, so the census says the three
// flush codes cannot arise; a flush the package may run keeps its three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`,
// `leaf-operation-unaccounted`), never silence, never a violation.

// ---- G1: a getter that flushes is on a prototype literal, and a receiver in the derived object's accessor reads it in a step. No reference of `base` is written in the step.
const protoSuperBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoSuper = {
  __proto__: protoSuperBase,
  get h(): number {
    void runStep(() => {
      void (super.g as any);
    });
    return 1;
  },
};
setTimeout(() => {
  void protoSuper.h;
});

const protoThisBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoThis = {
  __proto__: protoThisBase,
  get h(): number {
    void runStep(() => {
      void (this as any).g;
    });
    return 1;
  },
};
setTimeout(() => {
  void protoThis.h;
});

// the prototype installed after the literal, by a write of `__proto__`
const protoWrittenBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoWritten = {
  get h(): number {
    void runStep(() => {
      void (super.g as any);
    });
    return 1;
  },
};
(protoWritten as any).__proto__ = protoWrittenBase;
setTimeout(() => {
  void protoWritten.h;
});

// two levels: the getter that flushes is two prototypes up from the accessor that reads it, and the middle literal writes no receiver
const protoDeepBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoDeepMid = { __proto__: protoDeepBase };
const protoDeep = {
  __proto__: protoDeepMid,
  get h(): number {
    void runStep(() => {
      void (super.g as any);
    });
    return 1;
  },
};
setTimeout(() => {
  void protoDeep.h;
});

// a setter reads it through `super`
const protoSetterBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoSetter = {
  __proto__: protoSetterBase,
  set h(_v: number) {
    void runStep(() => {
      void (super.g as any);
    });
  },
};
setTimeout(() => {
  protoSetter.h = 1;
});

// the prototype is named by a conditional and by a call
const protoPickedBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const protoPicked = {
  __proto__: Math.random() > 2 ? protoPickedBase : {},
  get h(): number {
    void runStep(() => {
      void (super.g as any);
    });
    return 1;
  },
};
setTimeout(() => {
  void protoPicked.h;
});

// a method that flushes is on the prototype, and a receiver reads it
const protoMethodBase = {
  m(): void {
    flush();
  },
};
const protoMethod = {
  __proto__: protoMethodBase,
  get h(): number {
    void runStep(() => {
      (this as any).m();
    });
    return 1;
  },
};
setTimeout(() => {
  void protoMethod.h;
});

// ---- the neighbours, no prototype: a method of the literal is run through `this` in a closure a package runs, and no call edge reaches it
const ownMethod = {
  m(): void {
    flush();
  },
  run(): void {
    void runStep(() => {
      this.m();
    });
  },
};
setTimeout(() => {
  ownMethod.run();
});

// the method itself is handed to the step through `this`
const ownValue = {
  m(): void {
    flush();
  },
  run(): void {
    void runStep(this.m);
  },
};
setTimeout(() => {
  ownValue.run();
});

// the member is an arrow, read back by a destructuring of `this`
const ownArrow = {
  m: (): void => {
    flush();
  },
  run(): void {
    void runStep(() => {
      const { m } = this;
      m();
    });
  },
};
setTimeout(() => {
  ownArrow.run();
});

// a function written outside the literal is placed in it (`m: helper`), and a receiver reads it
function placedHelper(): void {
  flush();
}
const placed = {
  m: placedHelper,
  get h(): number {
    void runStep(() => {
      this.m();
    });
    return 1;
  },
};
setTimeout(() => {
  void placed.h;
});

// a prototype's members are spread into a literal that holds a receiver
const spreadBase = {
  m(): void {
    flush();
  },
};
const spread = {
  ...spreadBase,
  get h(): number {
    void runStep(() => {
      this.m();
    });
    return 1;
  },
};
setTimeout(() => {
  void spread.h;
});

// a method written outside the literal names `this` (the callee of a call that binds it), and reads a member in a step
function foreignReader(this: any) {
  void runStep(() => {
    this.m();
  });
}
const foreign = {
  m(): void {
    flush();
  },
  g: foreignReader,
};
setTimeout(() => {
  foreign.g();
});
const foreignBase = {
  m(): void {
    flush();
  },
};
const foreignDerived = { __proto__: foreignBase, g: foreignReader };
setTimeout(() => {
  foreignDerived.g();
});

// the method that names `this` is another module's, placed as a property's value and as a shorthand
const foreignImported = {
  m(): void {
    flush();
  },
  g: importedReader,
};
setTimeout(() => {
  foreignImported.g();
});
const foreignShort = {
  m(): void {
    flush();
  },
  importedReader,
};
setTimeout(() => {
  foreignShort.importedReader();
});

// the key of the write may be `__proto__`, which a file of the program spells
const keyedBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const keyed: any = {
  get h(): number {
    void runStep(() => {
      void (this as any).g;
    });
    return 1;
  },
};
const keyedName = (Math.random() > 2 ? "x" : "__proto__") as string;
keyed[keyedName] = keyedBase;
setTimeout(() => {
  void keyed.h;
});

// ---- twins, silent: nothing reads the flushing member through a receiver, or the read runs where no package runs it
// a prototype with a flushing getter, and a derived literal that writes no receiver
const twinProtoBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const twinProto = {
  __proto__: twinProtoBase,
  get h(): number {
    return 1;
  },
};
setTimeout(() => {
  void twinProto.h;
});
// the same, the prototype written after the literal
const twinWrittenBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const twinWritten = {
  get h(): number {
    void runStep(() => {
      void 1;
    });
    return 1;
  },
};
(twinWritten as any).__proto__ = twinWrittenBase;
setTimeout(() => {
  void twinWritten.h;
});
// a `null` prototype: the literal's receiver reads nothing of the getter that is read in a timer
const twinNullBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const twinNull = {
  __proto__: null,
  get h(): number {
    void runStep(() => {
      void (this as any).g;
    });
    return 1;
  },
};
setTimeout(() => {
  void twinNullBase.g;
  void twinNull.h;
});
// a receiver in a literal that holds no flushing member, beside a literal that holds one and is read in a timer
const twinUnrelatedFlush = {
  get g(): number {
    flush();
    return 1;
  },
};
const twinUnrelatedReader = {
  get h(): number {
    void runStep(() => {
      void (this as any).g;
    });
    return 1;
  },
};
setTimeout(() => {
  void twinUnrelatedFlush.g;
  void twinUnrelatedReader.h;
});
// a literal whose method flushes and which writes no receiver; a step runs a closure that never reads it
const twinOwn = {
  m(): void {
    flush();
  },
  run(): void {
    void runStep(() => {
      void 1;
    });
  },
};
setTimeout(() => {
  twinOwn.run();
  twinOwn.m();
});
