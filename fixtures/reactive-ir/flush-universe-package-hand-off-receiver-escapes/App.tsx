import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #229, B2: the receiver used as a value (kept in a local, read through a key the program does not name, reflected, spread, taken whole by a rest) may read any key,
// anywhere it goes, so a function stored under any key is handed when a package may run what reads it. `steplib` (its source is not read) runs what it is handed as an action's
// step; a flush it may run keeps its three uncertifiable rows, never silence, never a violation. One such use hands every stored function of the program, so a case here is proved
// alone by its pattern program (`scripts/fuzz/patterns/pkghand/late-*`); the twins are in `flush-universe-package-hand-off-receiver-reads-twins`.

// the receiver kept in a local, the step reads the member through it
const lateSelf = {
  get h(): number {
    const self = this;
    void runStep(() => {
      self.lateSelfM();
    });
    return 1;
  },
  lateSelfM: undefined as any,
};
lateSelf.lateSelfM = function () {
  flush();
};
setTimeout(() => {
  void lateSelf.h;
});

// a key the program does not name
const lateKey = "lateComputedM";
const lateComputed = {
  get h(): number {
    void runStep(() => {
      (this as any)[lateKey]();
    });
    return 1;
  },
  lateComputedM: undefined as any,
};
lateComputed.lateComputedM = function () {
  flush();
};
setTimeout(() => {
  void lateComputed.h;
});

// the receiver reflected
const lateReflect = {
  get h(): number {
    void runStep(() => {
      Reflect.get(this, "lateReflectM")();
    });
    return 1;
  },
  lateReflectM: undefined as any,
};
lateReflect.lateReflectM = function () {
  flush();
};
setTimeout(() => {
  void lateReflect.h;
});

// the receiver spread into a copy
const lateSpread = {
  get h(): number {
    void runStep(() => {
      const copy = { ...(this as any) };
      copy.lateSpreadM();
    });
    return 1;
  },
  lateSpreadM: undefined as any,
};
lateSpread.lateSpreadM = function () {
  flush();
};
setTimeout(() => {
  void lateSpread.h;
});

// a pattern with a rest takes every other member
const lateRest = {
  get h(): number {
    void runStep(() => {
      const { lateRestOther, ...rest } = this as any;
      void lateRestOther;
      rest.lateRestM();
    });
    return 1;
  },
  lateRestOther: undefined as any,
  lateRestM: undefined as any,
};
lateRest.lateRestM = function () {
  flush();
};
setTimeout(() => {
  void lateRest.h;
});

// a class's getter keeps the receiver in a local
class LateSelfClass {
  lateSelfClassM: any;
  get h(): number {
    const self = this;
    void runStep(() => {
      self.lateSelfClassM();
    });
    return 1;
  }
}
const lateSelfClass = new LateSelfClass();
lateSelfClass.lateSelfClassM = function () {
  flush();
};
setTimeout(() => {
  void lateSelfClass.h;
});

export function App() {
  return <p>x</p>;
}
