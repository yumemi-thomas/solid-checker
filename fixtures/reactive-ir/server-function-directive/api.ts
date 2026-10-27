"use server";
// SC7006 valid-use-server, module-level directive file (ADR 0427). Each export
// is registered by its evaluated value, so a wrapped export is fine; a
// re-export is a compile error (rc.8+), and a client-side declaration wrapper
// called at module scope never runs on the client.
import { GET } from "@solidjs/web/server-functions";
import { logged } from "./wrappers";

// A client-side wrapper at module scope: its behavior is lost — finding on the
// call. The export holding it is not a second finding.
export const getUser = GET(async (id: string) => {
  return { id };
});

// A server-side wrapper is the supported composition (RFC 10, export-value
// registration): the result is not shown to be a function by the facts, so the
// export is an uncertifiable result, never a violation.
export const audited = logged(async () => {
  return 1;
});

// Re-export: a compile error under a module-level directive — finding (one per
// specifier).
export { helper } from "./helpers";

// Star re-export: finding.
export * from "./more";

// Direct function declaration: registered — silent.
export async function addTodo(title: string) {
  return title;
}

// Direct function expression: registered — silent.
export const removeTodo = async (id: string) => {
  return id;
};

// Type-only exports are erased at build time — silent.
export type TodoId = string;
