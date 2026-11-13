import { flush } from "solid-js";
import { runStep } from "steplib";

// A function stored through a parameter's member (`target.m = function () { … }`) is stored into the object the callers pass, and the code that reads that object reads it by
// the caller's own name (`o.m()`), which the walk of the parameter does not reach. `steplib` (its source is not read: its manifest names no Solid package) runs what it is
// handed as an action's step. The program names no action, effect or leaf primitive, so the census says the three flush codes cannot arise; a flush the package may run keeps
// its three uncertifiable rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. No receiver
// (`this`, `super`) is written in this program.

// ---- the holder is the argument of the one call
function putTyped(target: { putTypedM(): void }) {
  target.putTypedM = function () {
    flush();
  };
}
const putTypedHolder = { putTypedM(): void {} };
putTyped(putTypedHolder);
void runStep(() => {
  putTypedHolder.putTypedM();
});

// the parameter is `any`, the holder an `any` object
function putAny(target: any) {
  target.putAnyM = function () {
    flush();
  };
}
const putAnyHolder: any = {};
putAny(putAnyHolder);
void runStep(() => {
  putAnyHolder.putAnyM();
});

// the argument is a parameter of the caller, whose own caller passes the holder
function putInner(target: { putChainM(): void }) {
  target.putChainM = function () {
    flush();
  };
}
function putOuter(holder: { putChainM(): void }) {
  putInner(holder);
}
const putChainHolder = { putChainM(): void {} };
putOuter(putChainHolder);
void runStep(() => {
  putChainHolder.putChainM();
});

// the holder is passed by a copy of its name
function putCopy(target: { putCopyM(): void }) {
  target.putCopyM = function () {
    flush();
  };
}
const putCopyHolder = { putCopyM(): void {} };
const putCopyAlias = putCopyHolder;
putCopy(putCopyAlias);
void runStep(() => {
  putCopyAlias.putCopyM();
});

// an arrow bound to a `const`, a rest parameter after the holder
const putArrow = (target: { putArrowM(): void }, ...rest: unknown[]) => {
  void rest;
  target.putArrowM = function () {
    flush();
  };
};
const putArrowHolder = { putArrowM(): void {} };
putArrow(putArrowHolder);
void runStep(() => {
  putArrowHolder.putArrowM();
});

// the parameter is a pattern: the holder is a member of the argument
function putPattern({ target }: { target: { putPatternM(): void } }) {
  target.putPatternM = function () {
    flush();
  };
}
const putPatternHolder = { putPatternM(): void {} };
putPattern({ target: putPatternHolder });
void runStep(() => {
  putPatternHolder.putPatternM();
});

// ---- twins, silent
// the argument is a fresh literal, reached through the parameter only
function twinFresh(target: { twinFreshM(): void }) {
  target.twinFreshM = function () {
    flush();
  };
}
twinFresh({ twinFreshM(): void {} });

// the holder is read on a timer's stack, by a member nothing calls
function twinTimer(target: { twinTimerM(): void }) {
  target.twinTimerM = function () {
    flush();
  };
}
const twinTimerHolder = { twinTimerM(): void {}, twinTimerOther: 1 };
twinTimer(twinTimerHolder);
setTimeout(() => {
  void twinTimerHolder.twinTimerOther;
});

export function App() {
  return <p>x</p>;
}
