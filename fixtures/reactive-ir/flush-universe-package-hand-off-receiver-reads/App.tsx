import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #229, B2: a member reaches a receiver (`this`, `super`) by no placement in the object literal that writes it, so the object that holds it is no reference the
// read follows. `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed as an action's step. The program names no action, effect or leaf
// primitive, so the census says the three flush codes cannot arise; a flush the package may run keeps its three uncertifiable rows (`flush-in-action-unaccounted`,
// `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. A function stored under a key that a receiver reads, in code a package may run, is
// handed: every key of a case is its own, and no file reads a key through a value (`this[key]`, `const self = this`: the next fixture), so what is not handed stays silent.

// ---- the member is stored after the literal: `o.m = function () { … }`
const lateAssign = {
  get h(): number {
    void runStep(() => {
      (this as any).lateAssignM();
    });
    return 1;
  },
  lateAssignM: undefined as any,
};
lateAssign.lateAssignM = function () {
  flush();
};
setTimeout(() => {
  void lateAssign.h;
});

// the same read by a setter, run by a write
const lateSetter = {
  set h(value: number) {
    void runStep(() => {
      (this as any).lateSetterM();
    });
  },
  lateSetterM: undefined as any,
};
lateSetter.lateSetterM = function () {
  flush();
};
setTimeout(() => {
  lateSetter.h = 1;
});

// an object stored after the literal, the getter runs a method of it
const lateNested = {
  get h(): number {
    void runStep(() => {
      (this as any).lateNestedSub.m();
    });
    return 1;
  },
  lateNestedSub: undefined as any,
};
lateNested.lateNestedSub = {
  m(): void {
    flush();
  },
};
setTimeout(() => {
  void lateNested.h;
});

// the member is written by a project function
const latePut = {
  get h(): number {
    void runStep(() => {
      (this as any).latePutM();
    });
    return 1;
  },
  latePutM: undefined as any,
};
function putLatePut(target: { latePutM: any }) {
  target.latePutM = function () {
    flush();
  };
}
putLatePut(latePut);
setTimeout(() => {
  void latePut.h;
});

// the member is handed on, not called: the read is a value the step receives
const lateValue = {
  get h(): number {
    void runStep((this as any).lateValueM);
    return 1;
  },
  lateValueM: undefined as any,
};
lateValue.lateValueM = function () {
  flush();
};
setTimeout(() => {
  void lateValue.h;
});

// the member is destructured from the receiver in the step
const lateDestr = {
  get h(): number {
    void runStep(() => {
      const { lateDestrM } = this as any;
      lateDestrM();
    });
    return 1;
  },
  lateDestrM: undefined as any,
};
lateDestr.lateDestrM = function () {
  flush();
};
setTimeout(() => {
  void lateDestr.h;
});

// a key the program writes as a string
const lateStrKey = {
  get h(): number {
    void runStep(() => {
      (this as any)["lateStrKeyM"]();
    });
    return 1;
  },
  lateStrKeyM: undefined as any,
};
lateStrKey["lateStrKeyM"] = function () {
  flush();
};
setTimeout(() => {
  void lateStrKey.h;
});

// the member that is read calls another stored under a key the step does not name
const lateChain = {
  get h(): number {
    void runStep(() => {
      (this as any).lateChainRun();
    });
    return 1;
  },
  lateChainRun: undefined as any,
  lateChainM: undefined as any,
};
lateChain.lateChainRun = function () {
  (this as any).lateChainM();
};
lateChain.lateChainM = function () {
  flush();
};
setTimeout(() => {
  void lateChain.h;
});

// ---- the receiver is on the prototype, the flushing member on the derived object
// a literal that writes no receiver: `{ __proto__: base, m() { … } }`
const protoReceiverBase = {
  get h(): number {
    void runStep(() => {
      (this as any).protoReceiverM();
    });
    return 1;
  },
};
const protoReceiver = {
  __proto__: protoReceiverBase,
  protoReceiverM(): void {
    flush();
  },
};
setTimeout(() => {
  void (protoReceiver as any).h;
});

// `Object.create(base)` and a member stored after
const protoCreateBase = {
  get h(): number {
    void runStep(() => {
      (this as any).protoCreateM();
    });
    return 1;
  },
};
const protoCreate = Object.create(protoCreateBase);
protoCreate.protoCreateM = function () {
  flush();
};
setTimeout(() => {
  void protoCreate.h;
});

// a class that reads the member, an instance that is given it
class ProtoClass {
  protoClassM: any;
  get h(): number {
    void runStep(() => {
      this.protoClassM();
    });
    return 1;
  }
}
const protoClass = new ProtoClass();
protoClass.protoClassM = function () {
  flush();
};
setTimeout(() => {
  void protoClass.h;
});

// ---- twins, silent
// the receiver reads a key no function that flushes is stored under
const twinOtherKey = {
  get h(): number {
    void runStep(() => {
      (this as any).twinReadM();
    });
    return 1;
  },
  twinReadM: undefined as any,
  twinStoredM: undefined as any,
};
twinOtherKey.twinReadM = function () {};
twinOtherKey.twinStoredM = function () {
  flush();
};
setTimeout(() => {
  void twinOtherKey.h;
});

// the receiver reads the key in a closure that runs on a timer's stack, which no package runs
const twinTimer = {
  get h(): number {
    setTimeout(() => {
      (this as any).twinTimerM();
    });
    return 1;
  },
  twinTimerM: undefined as any,
};
twinTimer.twinTimerM = function () {
  flush();
};
setTimeout(() => {
  void twinTimer.h;
});

// a member that no receiver reads, called by the code that holds it
const twinNoReader = {
  twinNoReaderM(): void {
    flush();
  },
};
setTimeout(() => {
  twinNoReader.twinNoReaderM();
});

export function App() {
  return <p>x</p>;
}
