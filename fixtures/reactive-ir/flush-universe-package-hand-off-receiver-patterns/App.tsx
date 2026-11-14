import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #232 (B3, B4): a parameter that is a pattern or a rest is no name for the receiver, and `Object.assign(this, …)` is a write through it. Each of these reads a key
// (or every key) where a function a package may run, so a function stored in a holder that may be a receiver is handed whatever key it is stored under (the read is `any`), and a
// holder that provably cannot be one (a plain `const` literal every use of which keeps it plain) is not: the twins at the end are silent beside them. `steplib` (its source is not
// read: its manifest names no Solid package) runs what it is handed as an action's step. The program names no action, effect or leaf primitive, so the census says the three flush
// codes cannot arise; a flush the package may run keeps its three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`,
// `leaf-operation-unaccounted`), never silence, never a violation.

// ---- B3: the pattern names the key, and the value it binds is called through `call`
function patternCallRead({ patternCallM }: any) {
  patternCallM.call(null);
}
const patternCall: any = {
  get g(): number {
    void runStep(() => {
      patternCallRead(this);
    });
    return 1;
  },
};
patternCall.patternCallM = function () {
  flush();
};
setTimeout(() => {
  void patternCall.g;
});

// ---- B3: the pattern alone runs a getter of the receiver
function patternGetterRead({ patternGetterM }: any) {
  void patternGetterM;
}
const patternGetterBase = {
  get g(): number {
    void runStep(() => {
      patternGetterRead(this);
    });
    return 1;
  },
};
const patternGetter = {
  __proto__: patternGetterBase,
  get patternGetterM(): number {
    flush();
    return 1;
  },
};
setTimeout(() => {
  void (patternGetter as any).g;
});

// ---- B3: an array pattern runs the receiver's iterator
function patternArrayRead([first]: any) {
  void first;
}
const patternArrayBase = {
  get g(): number {
    void runStep(() => {
      patternArrayRead(this);
    });
    return 1;
  },
};
const patternArray = {
  __proto__: patternArrayBase,
  *[Symbol.iterator]() {
    flush();
  },
};
setTimeout(() => {
  void (patternArray as any).g;
});

// ---- B3: a rest parameter is an array that holds the receiver
function restParamRead(...received: any[]) {
  received[0].restParamM();
}
const restParam: any = {
  get g(): number {
    void runStep(() => {
      restParamRead(this);
    });
    return 1;
  },
};
restParam.restParamM = function () {
  flush();
};
setTimeout(() => {
  void restParam.g;
});

// ---- B3: and a rest parameter that passes the receiver on
function restPassInner(given: any) {
  given.restPassM();
}
function restPassOuter(...received: any[]) {
  restPassInner(received[0]);
}
const restPass: any = {
  get g(): number {
    void runStep(() => {
      restPassOuter(this);
    });
    return 1;
  },
};
restPass.restPassM = function () {
  flush();
};
setTimeout(() => {
  void restPass.g;
});

// ---- B4: `Object.assign(this, …)` writes every key of its sources through `[[Set]]`, which runs the setter of the derived literal
const assignThisBase = {
  get g(): number {
    void runStep(() => {
      Object.assign(this, { assignThisK: 1 });
    });
    return 1;
  },
};
const assignThis = {
  __proto__: assignThisBase,
  set assignThisK(value: number) {
    void value;
    flush();
  },
};
setTimeout(() => {
  void (assignThis as any).g;
});

// ---- twins, silent: a plain `const` literal no use of which gives it a prototype, a receiver or a name nothing follows is no receiver, whatever the program's receivers read
const twinPlain = {
  twinPlainM(): void {
    flush();
  },
};
void twinPlain;
const twinNested = {
  twinNestedSub: {
    twinNestedM(): void {
      flush();
    },
  },
};
void twinNested.twinNestedSub;
function twinLeafRead({ twinLeafCount }: { twinLeafCount: number }): number {
  return twinLeafCount;
}
const twinLeaf = {
  twinLeafCount: 1,
  twinLeafM(): void {
    flush();
  },
};
twinLeafRead(twinLeaf);

export function App() {
  return <p>x</p>;
}
