import { flush } from "solid-js";

// ADR 1207: the program names no action, no deferring primitive and no leaf scope, so a flush no package can run ends by the program censuses. A
// function a literal holds under a key it owns, handed whole to a scheduler (`setTimeout(cb1.cb)`), is never run by a package: the literal is read only
// through keys it owns, so no accessor `Object.prototype` may hold runs against it, and the read gives out the function, never the object.

// Clean: the slot's function handed whole to a timer.
export function HandedToTimer() {
  const cb1 = {
    cb: () => {
      flush();
    },
  };
  setTimeout(cb1.cb, 0);
  return <div />;
}

// Clean: the member called after it is read whole too (an inert function, no receiver read).
export function HandedAndCalled() {
  const cb1 = {
    cb: () => {
      flush();
    },
  };
  setTimeout(cb1.cb, 0);
  cb1.cb();
  return <div />;
}

declare function keep(value: object): void;

// Uncertifiable: the object itself is handed to a function nothing here reads.
export function ObjectHanded() {
  const cb1 = {
    cb: () => {
      flush();
    },
  };
  keep(cb1);
  setTimeout(cb1.cb, 0);
  return <div />;
}

// Uncertifiable: a key the literal does not own is read (an accessor `Object.prototype` may hold would run against the object).
export function ForeignKeyRead() {
  const cb1: { cb: () => void; other?: unknown } = {
    cb: () => {
      flush();
    },
  };
  void cb1.other;
  setTimeout(cb1.cb, 0);
  return <div />;
}
