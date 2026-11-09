import { describe, expect, it } from "vitest";
import { ERROR_EVENT_CAP, errorListeners } from "./chrome-host/error-listeners.mjs";

// The census's `flw-settled-report-error` shape, without Solid: a window `error` listener whose work raises the next error event (in Chrome
// a microtask later, through `onSettled`'s throw from the flush; here at once). Before the cap no record came back from it: `harness`.
function page({ cap } = {}) {
  const target = new EventTarget();
  const thrown = [];
  const own = () => {};
  const listeners = errorListeners({ own, onThrow: (e) => thrown.push(e), ...(cap === undefined ? {} : { cap }) });
  const add = (listener) => target.addEventListener("error", listeners.wrapFor(listener));
  const raise = () => target.dispatchEvent(new Event("error"));
  return { target, thrown, own, listeners, add, raise };
}

describe("the Chrome host's error listeners", () => {
  it("end a listener that raises the next error: it sees ERROR_EVENT_CAP events, the record counts the rest", () => {
    const host = page();
    let ran = 0, harness = 0;
    host.target.addEventListener("error", () => { harness += 1; });
    host.add((event) => event.preventDefault());
    host.add(() => { ran += 1; host.raise(); });
    host.raise();
    expect(ran).toBe(ERROR_EVENT_CAP);
    expect(harness).toBe(ERROR_EVENT_CAP + 1);
    expect(host.listeners.state.withheld).toBe(1);
  });
  it("leave a page under the cap as it is: every listener sees every event, a throw is recorded and rethrown", () => {
    const host = page({ cap: 3 });
    const seen = [];
    host.add(() => seen.push("a"));
    host.add({ handleEvent: () => seen.push("b") });
    host.raise();
    host.raise();
    expect(seen).toEqual(["a", "b", "a", "b"]);
    const throwing = host.listeners.wrapFor(() => { throw new Error("listener"); });
    expect(() => throwing(new Event("error"))).toThrow("listener");
    expect(host.thrown.map((e) => e.message)).toEqual(["listener"]);
    expect(host.listeners.state).toEqual({ seen: 3, withheld: 0 });
  });
  it("keep the harness's own listener and one wrapper a listener", () => {
    const host = page();
    expect(host.listeners.wrapFor(host.own)).toBe(host.own);
    const listener = () => {};
    expect(host.listeners.wrapFor(listener)).toBe(host.listeners.wrapFor(listener));
    expect(host.listeners.wrapped.get(listener)).toBe(host.listeners.wrapFor(listener));
  });
});
