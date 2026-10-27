"use server";
import { GET, live, withMeta } from "@solidjs/web/server-functions";
import { query, action as submit, liveQuery } from "@solidjs/router";

// Positive: each is a call to an imported client-side wrapper at module scope.
export const a = GET(async () => 1);
export const b = live(async () => 2);
export const c = withMeta(async () => 3, { method: "GET" });
export const d = query(async () => 4, "d");
export const e = submit(async () => 5);
export const f = liveQuery(async () => 6, "f");

// Negative: a call inside a function is server-side code.
export async function setup() {
  return query(async () => 7, "g");
}
