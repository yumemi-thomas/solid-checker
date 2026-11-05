// Negative: each of these compiles (rc.21 transformDirectives, both builds).
const top = 1;
let topLet = 2;

export function ModuleBindings() {
  async function f() {
    "use server";
    return top + topLet;
  }
  return f;
}

export function OwnLocals() {
  async function f() {
    "use server";
    const b = 2;
    const g = () => b;
    return g();
  }
  return f;
}

export function TypeOnly() {
  type T = number;
  async function f(): Promise<T> {
    "use server";
    return 1 as T;
  }
  return f;
}

// A module-level server function referencing module scope.
export async function atTop() {
  "use server";
  return top;
}
