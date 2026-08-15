"use server";
// SC7006 server-function-module-directive: under a module-level directive,
// only direct function exports become client references (RFC 10 §Compiler
// implications) — wrapped exports, non-function default expressions, and
// re-exports are silently dropped from the client build.
import { GET } from "@solidjs/web/server-functions";
import { logged } from "./wrappers";

// Wrapped export: dropped from the client build — finding.
export const getUser = GET(async (id: string) => {
  return { id };
});

// A second wrapper shape, same drop — finding.
export const audited = logged(async () => {
  return 1;
});

// Re-export: dropped — finding (one per specifier).
export { helper } from "./helpers";

// Star re-export: dropped — finding.
export * from "./more";

// Direct function declaration: becomes a reference — silent.
export async function addTodo(title: string) {
  return title;
}

// Direct function expression: a reference too — silent.
export const removeTodo = async (id: string) => {
  return id;
};

// Type-only exports are erased at build time — silent.
export type TodoId = string;
