// A hoisted function declaration runs whenever it is called, even before a
// `return` that precedes it in the body.
export function redefines(p: any): unknown {
  g();
  return typeof p === "function" && !p.length ? p() : 0;
  function g() {
    Object.defineProperty(p, "length", { value: 1 });
  }
}

export function callsLater(p: () => number): number {
  return g();
  function g() {
    return p();
  }
}
