import { flush } from "solid-js";
import { id, runStep } from "steplib";
import { imported } from "./holder";

// ADR 0980. A function stored into an object (`X.m = function () { flush(); }`) is followed in every read of the name X, and a name is not the object: another name of the
// same object is never read. `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed as an action's step, so a store the build
// cannot prove stays inside the project keeps its three uncertifiable rows (SC2006, SC2008, SC3001), never silence and never a violation. The holder is followed only when it
// is sole-named: a `const` (or a `let` nothing reassigns) bound to a fresh object, every use of which is the object of a member access or an operand nothing takes.

// ---- positives: the store is made through another name of the object (Chrome: FLUSH_IN_ACTION; main certified each with no row)

// S2: a copy made by the initializer of another name
const s2o: any = {};
const s2a = s2o;
s2a.m2 = function () {
  flush();
};
void runStep(() => {
  s2o.m2();
});

// a copy made by an assignment
const letO: any = {};
let letA: any;
letA = letO;
letA.m3 = function () {
  flush();
};
void runStep(() => {
  letO.m3();
});

// the loop variable of an iteration of an array literal
const forO: any = {};
for (const x of [forO]) {
  x.m4 = function () {
    flush();
  };
}
void runStep(() => {
  forO.m4();
});

// a01: a getter returns the object
const getO: any = {};
const getBox = {
  get o() {
    return getO;
  },
};
getBox.o.m5 = function () {
  flush();
};
void runStep(() => {
  getO.m5();
});

// a08: a computed store through an alias
const computedO: any = {};
const computedA = computedO;
computedA["m6"] = function () {
  flush();
};
void runStep(() => {
  computedO.m6();
});

// a12: an instance of a class the project declares, through an alias
class Holder {
  m7: any;
}
const classO = new Holder();
const classA = classO;
classA.m7 = function () {
  flush();
};
void runStep(() => {
  classO.m7();
});

// a14: a comma expression
const sequenceO: any = {};
const sequenceA = (0, sequenceO);
sequenceA.m8 = function () {
  flush();
};
void runStep(() => {
  sequenceO.m8();
});

// a15: a conditional
const condO: any = {};
const condA = Math.random() < 2 ? condO : null;
condA.m9 = function () {
  flush();
};
void runStep(() => {
  condO.m9();
});

// a16: a nullish expression
const nullishO: any = {};
const nullishA = nullishO ?? null;
nullishA.m10 = function () {
  flush();
};
void runStep(() => {
  nullishO.m10();
});

// a17: a destructuring of an object literal that holds it
const destrLitO: any = {};
const { o: destrLitA } = { o: destrLitO };
destrLitA.m11 = function () {
  flush();
};
void runStep(() => {
  destrLitO.m11();
});

// a18: a proxy of it (a store through a proxy with no traps writes the target)
const proxyO: any = {};
const proxyA = new Proxy(proxyO, {});
proxyA.m12 = function () {
  flush();
};
void runStep(() => {
  proxyO.m12();
});

// the member path: the other name is a member of an object that holds it
const pathO: any = {};
const pathHolder = { o: pathO };
const pathA = pathHolder.o;
pathA.m13 = function () {
  flush();
};
void runStep(() => {
  pathO.m13();
});

// a destructured binding of an object that holds it
const patternO: any = {};
const patternHolder = { o: patternO };
const { o: patternA } = patternHolder;
patternA.m14 = function () {
  flush();
};
void runStep(() => {
  patternO.m14();
});

// a store through a chain of members: the object stored into is `box.o`, not `box`
const chainO: any = {};
const chainBox = { o: chainO };
chainBox.o.m15 = function () {
  flush();
};
void runStep(() => {
  chainO.m15();
});

// ---- ratchets: forms that already had a row on main and keep it (the store is made through the object's own name, which something else copies or reads)

// the run side reads the object through a closure
const closureO: any = {};
const closureGet = () => closureO;
closureO.m16 = function () {
  flush();
};
void runStep(() => {
  closureGet().m16();
});

// through an array literal
const arrayO: any = {};
arrayO.m17 = function () {
  flush();
};
void runStep(() => {
  [arrayO][0].m17();
});

// through a Map
const mapO: any = {};
const mapHolder = new Map<string, any>([["k", mapO]]);
mapO.m18 = function () {
  flush();
};
void runStep(() => {
  mapHolder.get("k").m18();
});

// the store is an `Object.assign` through an alias
const assignO: any = {};
const assignA = assignO;
Object.assign(assignA, {
  m19: function () {
    flush();
  },
});
void runStep(() => {
  assignO.m19();
});

// the store is `Object.defineProperty` through an alias
const defineO: any = {};
const defineA = defineO;
Object.defineProperty(defineA, "m20", {
  value: function () {
    flush();
  },
});
void runStep(() => {
  defineO.m20();
});

// the store is `Reflect.set` through an alias
const reflectO: any = {};
const reflectA = reflectO;
Reflect.set(reflectA, "m21", function () {
  flush();
});
void runStep(() => {
  reflectO.m21();
});

// the run side reads the member through `this` in a method of the object
const thisO: any = {
  run() {
    this.m22();
  },
};
thisO.m22 = function () {
  flush();
};
void runStep(() => {
  thisO.run();
});

// the run side reads the argument through `arguments`
function argumentsCall(_holder?: unknown) {
  arguments[0].m23();
}
const argumentsO: any = {};
argumentsO.m23 = function () {
  flush();
};
void runStep(() => {
  argumentsCall(argumentsO);
});

// the run side reads it through a weak reference
const weakO: any = {};
const weakRef = new WeakRef(weakO);
weakO.m24 = function () {
  flush();
};
void runStep(() => {
  weakRef.deref().m24();
});

// the run side reads it through an alias
const runAliasO: any = {};
const runAliasA = runAliasO;
runAliasO.m25 = function () {
  flush();
};
void runStep(() => {
  runAliasA.m25();
});

// the run side reads it through a getter
const runGetterO: any = {};
const runGetterBox = {
  get o() {
    return runGetterO;
  },
};
runGetterO.m26 = function () {
  flush();
};
void runStep(() => {
  runGetterBox.o.m26();
});

// the package hands it back (`id`: its source is not read)
const idO: any = {};
idO.m27 = function () {
  flush();
};
void runStep(() => {
  id(idO).m27();
});

// the store goes through a cast
const castO: any = {};
(castO as any).m28 = function () {
  flush();
};
void runStep(() => {
  castO.m28();
});

// an import of another module's object
imported.m29 = function () {
  flush();
};
void runStep(() => {
  imported.m29();
});

// the store is made inside the step
const insideO: any = {};
void runStep(() => {
  insideO.m30 = function () {
    flush();
  };
  insideO.m30();
});

// a module export of an alias of the object
const exportO: any = {};
export const exportA = exportO;
exportO.m31 = function () {
  flush();
};
void runStep(() => {
  exportO.m31();
});

// the holder is sole-named and the step reads the member: followed, and handed
const readO: any = {};
readO.m37 = function () {
  flush();
};
void runStep(() => {
  readO.m37();
});

// ---- negatives, silent: the holder is sole-named and the function is never run by a package
// a `const` bound to a fresh object, written and read in the project (a call `soleO.m()` is not silent: the member is replaced, so the callee is no one function the
// call graph resolves, and a callee it does not resolve may read a member through `this`)
const soleO: any = {};
soleO.m32 = function () {
  flush();
};
void soleO.m32;

// the same, and the package step runs something else
const twinO: any = {};
twinO.m33 = function () {
  flush();
};
void runStep(() => {
  void twinO.other;
});

// a `let` nothing reassigns
let soleLet: any = {};
soleLet.m34 = function () {
  flush();
};
void soleLet.m34;

// an array literal holder
const soleArray: any = [];
soleArray.m35 = function () {
  flush();
};
void soleArray.m35;

// the holder is a typeof operand and compared
const soleInert: any = {};
soleInert.m36 = function () {
  flush();
};
void (typeof soleInert === "object" && soleInert !== null);

export function App() {
  return <p>x</p>;
}
