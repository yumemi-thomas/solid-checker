// ADR 0706: each server function below captures a binding an enclosing
// function declares; rc.21's directive pass rejects every one with "server
// functions cannot capture non-top-level variables" (client and server).

export function EnclosingConst() {
  const a = 1;
  async function f() {
    "use server";
    return a;
  }
  return f;
}

export function EnclosingParam(p: number) {
  async function f() {
    "use server";
    return p;
  }
  return f;
}

export function DefaultParam() {
  const a = 1;
  async function f(x = a) {
    "use server";
    return x;
  }
  return f;
}

export function ArrowConst() {
  const a = 1;
  const f = async () => {
    "use server";
    return a;
  };
  return f;
}

export function EnclosingHelper() {
  function h() {
    return 1;
  }
  async function f() {
    "use server";
    return h();
  }
  return f;
}

export function SelfReference() {
  async function f(): Promise<number> {
    "use server";
    return f.length;
  }
  return f;
}

export function Sibling() {
  async function g() {
    "use server";
    return 1;
  }
  async function f() {
    "use server";
    return g();
  }
  return f;
}
