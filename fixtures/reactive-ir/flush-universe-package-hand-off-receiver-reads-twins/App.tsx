import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #229, B2: what stays silent. A receiver used as a value in a function nothing hands a package (and followed to a function that reads nothing of it), a key read by
// a function nothing hands a package, a function stored under a computed key where no receiver is read by code a package may run: the flushes keep no row.

// the receiver kept in a local, in a function no reference names, and passed on to a function that reads nothing of it
type TwinEscape = { twinEscapeM: () => void };
function twinConsume(object: TwinEscape): void {
  void object;
}
function twinNeverRun(this: TwinEscape): void {
  const self = this;
  twinConsume(self);
}
const twinEscape: TwinEscape = { twinEscapeM: () => {} };
twinEscape.twinEscapeM = function () {
  flush();
};
setTimeout(() => {
  void twinEscape;
});

// a key a function nothing hands a package reads through `this`
function twinNeverReads(this: { twinReadM: () => void }): void {
  this.twinReadM();
}
const twinRead: { twinReadM: () => void } = { twinReadM: () => {} };
twinRead.twinReadM = function () {
  flush();
};
setTimeout(() => {
  void twinRead;
});

// a function stored under a computed key, and nothing a package runs reads a receiver
const twinKeyBase = {
  get g(): number {
    flush();
    return 1;
  },
};
const twinKeyTarget: any = {};
const twinKey = "twin" + "Key";
twinKeyTarget[twinKey] = twinKeyBase;
setTimeout(() => {
  void twinKeyTarget.g;
});

// a package takes the step, and no receiver is read
void runStep(() => {});

export function App() {
  return <p>x</p>;
}
