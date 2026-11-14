import { flush } from "solid-js";
import { id, runStep } from "steplib";

// ADR 0980. A function stored through a parameter's member (`t.m = function () { flush(); }`) is stored into the object the callers pass, so it is followed to each caller's
// argument (ADR 0967 amended), and only when the callers are all known and the argument is the name of a binding that is sole-named except for that call (the rule is stated
// in the fixture `flush-holder-alias`). `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed as an action's step, so a flush it may
// run keeps its three uncertifiable rows (SC2006, SC2008, SC3001), never silence and never a violation. Each site has its own key.

// ---- positives (Chrome: FLUSH_IN_ACTION; main certified each of them with no row, except the five marked "on main too")

// B6(a): the argument is another name of the object, made before the call
function putAlias(t: any) {
  t.alias1 = function () {
    flush();
  };
}
const aliasO: any = {};
const aliasA = aliasO;
putAlias(aliasA);
void runStep(() => {
  aliasO.alias1();
});

// B6(a): the argument is a parameter of the caller that holds a copy of the object it was given
function putOuterCopy(t: any) {
  t.outer2 = function () {
    flush();
  };
}
function outerCopy(h: any) {
  const copy = h;
  putOuterCopy(copy);
}
const outerO: any = {};
outerCopy(outerO);
void runStep(() => {
  outerO.outer2();
});

// on main too: the argument is a call's result (`same` returns what it is given)
function putId(t: any) {
  t.id3 = function () {
    flush();
  };
}
function same(x: any) {
  return x;
}
const idO: any = {};
putId(same(idO));
void runStep(() => {
  idO.id3();
});

// on main too: the argument is a package's result
function putPackageId(t: any) {
  t.package4 = function () {
    flush();
  };
}
const packageO: any = {};
putPackageId(id(packageO));
void runStep(() => {
  packageO.package4();
});

// B6(a): a class instance, passed by another name
class Holder {
  class5: any;
}
function putClass(t: any) {
  t.class5 = function () {
    flush();
  };
}
const classO = new Holder();
const classA = classO;
putClass(classA);
void runStep(() => {
  classO.class5();
});

// B6(b): a copy of the parameter inside the function
function putInner(t: any) {
  const s = t;
  s.inner6 = function () {
    flush();
  };
}
const innerO: any = {};
putInner(innerO);
void runStep(() => {
  innerO.inner6();
});

// B6(b): the parameter is reassigned to another parameter (the store goes into the second argument)
function putReassign(t: any, u: any) {
  t = u;
  t.reassign7 = function () {
    flush();
  };
}
const reassignO: any = {};
putReassign({}, reassignO);
void runStep(() => {
  reassignO.reassign7();
});

// B6(c): a default value receives the store (the call passes nothing)
const sharedDefault: any = {};
function putDefault(t: any = sharedDefault) {
  t.default8 = function () {
    flush();
  };
}
putDefault();
void runStep(() => {
  sharedDefault.default8();
});

// B6(c): a spread stands before the index
function putSpread(first: any, t: any) {
  void first;
  t.spread9 = function () {
    flush();
  };
}
const spreadO: any = {};
const spreadArguments: [any, any] = [{}, spreadO];
putSpread(...spreadArguments);
void runStep(() => {
  spreadO.spread9();
});

// B6(d): the function is a method of an object literal
const api = {
  put(t: any) {
    t.method10 = function () {
      flush();
    };
  },
};
const methodO: any = {};
api.put(methodO);
void runStep(() => {
  methodO.method10();
});

// on main too: the function is a function expression passed on, never bound to a name
const unnamedO: any = {};
[
  function (t: any) {
    t.unnamed11 = function () {
      flush();
    };
  },
].forEach((put) => put(unnamedO));
void runStep(() => {
  unnamedO.unnamed11();
});

// on main too: the callers are not all direct calls (one is an argument)
function putEscapes(t: any) {
  t.escapes12 = function () {
    flush();
  };
}
const escapesO: any = {};
putEscapes(escapesO);
[escapesO].forEach(putEscapes);
void runStep(() => {
  escapesO.escapes12();
});

// the argument is a member of an object that holds the object
const memberO: any = {};
const memberHolder = { o: memberO };
function putMember(t: any) {
  t.member13 = function () {
    flush();
  };
}
putMember(memberHolder.o);
void runStep(() => {
  memberO.member13();
});

// ---- ratchets

// on main too: the holder is sole-named and the step reads the member
function putSole(t: any) {
  t.sole14 = function () {
    flush();
  };
}
const soleO: any = {};
putSole(soleO);
void runStep(() => {
  soleO.sole14();
});

// the argument is the parameter of an outer function, whose caller passes the holder
function putChainInner(t: any) {
  t.chain15 = function () {
    flush();
  };
}
function putChainOuter(h: any) {
  putChainInner(h);
}
const chainO: any = {};
putChainOuter(chainO);
void runStep(() => {
  chainO.chain15();
});

// ---- negatives, silent: the argument is sole-named and the step never reads the member
function putNever(t: any) {
  t.never16 = function () {
    flush();
  };
}
const neverO: any = {};
putNever(neverO);
void runStep(() => {
  void neverO.other;
});

// the same, and no package step at all
function putQuiet(t: any) {
  t.quiet17 = function () {
    flush();
  };
}
const quietO: any = {};
putQuiet(quietO);
void quietO.quiet17;

// a chain of two functions
function putNeverInner(t: any) {
  t.neverInner18 = function () {
    flush();
  };
}
function putNeverOuter(h: any) {
  putNeverInner(h);
}
const neverChainO: any = {};
putNeverOuter(neverChainO);
void runStep(() => {
  void neverChainO.other;
});

// a fresh object literal is the argument: nothing else reaches it
function putFresh(t: any) {
  t.fresh19 = function () {
    flush();
  };
}
putFresh({});

export function App() {
  return <p>x</p>;
}
