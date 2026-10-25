// A string statement is a directive only in the prologue. Each of these keeps
// its function on the client — finding on the statement.
export async function afterStatement(id: string) {
  const table = "users";
  "use server";
  return [table, id];
}

export async function inIf(id: string) {
  if (id) {
    "use server";
    return id;
  }
  return "";
}

export async function inLoop(ids: string[]) {
  for (const id of ids) {
    "use server";
    void id;
  }
}

export async function inBareBlock() {
  {
    "use server";
  }
}

// Negative: another directive may precede it in the prologue.
export async function afterUseStrict(id: string) {
  "use strict";
  "use server";
  return id;
}

// Negative: a string in expression position is just a string.
export const mode = "use server";
export function describeMode() {
  return "use server";
}
