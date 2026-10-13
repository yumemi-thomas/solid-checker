import { kept } from "./kept";
export type Kept = typeof kept;

// `@other` resolves to other.ts, `./other.js` to the same file, and `fs` is a
// Node built-in: none of them exposes anything of kept.ts.
export async function later(item: { label(): string }) {
  kept(item);
  const module = await import("@other");
  const same = await import("./other.js");
  const fs = require("fs");
  return [module.answer, same.answer, fs];
}
