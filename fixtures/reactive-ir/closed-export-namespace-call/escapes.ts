import * as listing from "./enumerated";
import * as picking from "./picked";
import { loaded } from "./loaded";
// The type query makes the direct call the only remaining entry (ADR 0219).
export type Loaded = typeof loaded;
import * as reexported from "./barrel2";
import * as defaulted from "./barrel3";
import { viaDefault } from "./defaulted";

// Each module is entered by a visible call, and also through its namespace
// object, which names no export.
export function enumerate(item: { label(): string }) {
  listing.listed({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(listing)) result = f(item);
  return result;
}
export function pick(item: { label(): string }) {
  picking.picked({ label() { return "fixed"; } });
  const f = picking["picked"];
  return f(item);
}
export async function load(item: { label(): string }) {
  loaded({ label() { return "fixed"; } });
  const module = await import("./loaded");
  let result = "";
  for (const f of Object.values(module)) result = f(item);
  return result;
}
export function enumerateBarrel(item: { label(): string }) {
  reexported.viaBarrel({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(reexported)) result = f(item);
  return result;
}
export type ViaDefault = typeof viaDefault;
export function enumerateDefault(item: { label(): string }) {
  viaDefault({ label() { return "fixed"; } });
  let result = "";
  for (const f of Object.values(defaulted)) result = f(item);
  return result;
}
