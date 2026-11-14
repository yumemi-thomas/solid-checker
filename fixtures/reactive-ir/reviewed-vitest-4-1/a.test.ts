import { step } from "fz-step";
import { expect, vi } from "vitest";

// Module scope has no owner: nothing here is a site.
const onLoad = vi.fn();
const text: string = "abc";
const count = 3;
const box = { a: 1, run() {} };
const items = [1, 2];
enum Color {
  Red,
  Blue,
}
declare const maybe: string | undefined;
declare const mixed: string | { a: number };
declare const anything: any;
declare const unknownValue: unknown;

// The reviewed reading (ADR 0974) clears a matcher it names, handed a value no foreign accessor can run in: a primitive by the compiler's facts, a flat object
// literal of primitives, and for the spy matchers a spy `vi.fn` made.
export function Primitives(other: number) {
  expect(count).toBe(3);
  expect(text).toEqual("abc");
  expect(other).toBeGreaterThan(count);
  expect(text.length).toBeGreaterThanOrEqual(1);
  expect(maybe).toBeUndefined();
  expect(text).toMatch("b");
  expect(Boolean(text)).toBeTruthy();
  expect(Color.Red).toBe(0);
}
// A type parameter's value has its constraint's primitive domain, and the printed type may not be read for a decision (ADR 0367): one constrained to a primitive clears.
export function Constrained<T extends string>(value: T) {
  expect(value).toBe("a" as T);
}
export function Literals() {
  expect({ a: 1, b: "s" }).toEqual({ a: 1, b: "s" });
  expect({ a: 1 }).toStrictEqual({ a: 1 });
}
export function Spies() {
  const spy = vi.fn((n: number) => n);
  expect(onLoad).toHaveBeenCalledTimes(0);
  expect(onLoad).toHaveBeenCalled();
  expect(spy).toHaveBeenCalledOnce();
}
// A registration stores what it is handed: `vi.fn(impl)` keeps the function written there, `vi.stubGlobal(name, value)` defines the value.
export function Registrations() {
  const mock = vi.fn();
  const mocked = vi.fn(() => 1);
  const anonymous = vi.fn(function () {});
  vi.stubGlobal("stubbed", box);
  return [mock, mocked, anonymous];
}

// The registration clears the call, not the value: a package function it stores runs wherever the global is called, so handing one to it is a site as before.
export function StoredPackageFunction() {
  vi.stubGlobal("stepped", step);
}

// Everything else keeps its site: an object, an array, an unconstrained type parameter's value, a union with an object, an `any` or an `unknown`, a literal that holds another
// object, an expected value that is an object, a matcher the reading does not name, a `.not` chain, a spy handed to a matcher that reads its calls' arguments.
export function Objects() {
  expect(box).toEqual({ a: 1, run() {} });
  expect(items).toEqual([1, 2]);
  expect({ a: { b: 1 } }).toEqual({ a: { b: 1 } });
  expect({ a: 1, run() {} }).toEqual(box);
  expect(mixed).toBe("x");
}
export function Unknowns<T>(value: T) {
  expect(value).toBe(value);
  expect(anything).toBe(1);
  expect(unknownValue).toBe(1);
}
export function Unnamed() {
  expect(count).not.toBeNull();
  expect(text).toHaveLength(3);
  expect(onLoad).toHaveBeenCalledWith(box);
  expect(onLoad).not.toHaveBeenCalled();
}
// A later call through a mock is a site of its own, and so is a registration that reads what it is given.
export function Mocks() {
  onLoad(1);
  const wrapped = vi.fn(onLoad);
  const spied = vi.spyOn(box, "run");
  return [wrapped, spied];
}
