import { createContext, useContext } from "solid-js";
import { boxedElsewhere } from "./box";

// ADR 1059: a function a helper boxes (ADR 1058: `function boxed(cb) { return { cb } }`, `const box = boxed(f)`) runs where
// `box.cb()` is called, so the owner graph gives `f` that call as its caller. `useContext` there reads the owner of the call:
// a component body's (a context never provided throws ContextNotFoundError, SC4002), or none from a timer (SC4001).
const Bare = createContext<number>();
const Seven = createContext(7);
function boxed(cb: () => void) {
  return { cb };
}

// Violations.
export function BodyContextNotFound() {
  const box = boxed(() => {
    useContext(Bare);
  });
  box.cb();
  return <p />;
}
export function TimerNoOwner() {
  const box = boxed(() => {
    useContext(Seven);
  });
  setTimeout(() => {
    box.cb();
  }, 0);
  return <p />;
}

// Clean: in the body, a context with a default reads it under the component's owner.
export function BodyWithDefault() {
  const box = boxed(() => {
    useContext(Seven);
  });
  box.cb();
  return <p />;
}

// Obligations (uncertifiable): the box is not a closed literal, so its member may run anything anywhere.
function hand(box: { cb: () => void }) {
  void box;
}
export function TimerBoxHandedOn() {
  const box = boxed(() => {
    useContext(Seven);
  });
  hand(box);
  setTimeout(() => {
    box.cb();
  }, 0);
  return <p />;
}
export function TimerImportedHelper() {
  const box = boxedElsewhere(() => {
    useContext(Seven);
  });
  setTimeout(() => {
    box.cb();
  }, 0);
  return <p />;
}

// Uncertifiable: a hoisted declaration calls the box before its declarator ran; that call throws a ReferenceError
// (caught here), so the boxed function never runs (ADR 0299's order).
export function BodyBeforeDeclarator() {
  function enter() {
    box.cb();
  }
  try {
    enter();
  } catch {
    // swallowed
  }
  const box = boxed(() => {
    useContext(Bare);
  });
  return <p />;
}
