"use server";
import * as router from "@solidjs/router";
import { query as mine } from "./mine";
import { query as other } from "some-other-package";

// Negative: a member call names no import binding.
const a = router.query(async () => 1, "a");
// Negative: the import is not from a Solid package.
const b = mine(async () => 2, "b");
const c = other(async () => 3, "c");

// Negative: a project function spelled like a wrapper.
function GET<T>(fn: T): T {
  return fn;
}
const d = GET(async () => 4);

export async function use() {
  return [a, b, c, d];
}
