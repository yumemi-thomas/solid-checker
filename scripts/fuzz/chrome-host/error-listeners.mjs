// The Chrome host's wrapper around the program's window `error` listeners (runner.tsx). What a listener throws is recorded, then
// rethrown, so the page sees what it would have (Part 16, B2: a throw inside an `error` listener is never dispatched to the error
// listeners, so only the wrapper sees it).
//
// A listener that raises the next error it will handle never lets the page return to the event loop: `onSettled(() => () => {})` called
// from the listener, unowned, throws SETTLED_CLEANUP_UNOWNED from the next flush, a microtask; the throw is reported, the listener
// enqueues the next one, and Chrome loops in one microtask checkpoint (the census's `flw-settled-report-error`; the oracle's page never
// binds either). No record came back, at any budget: `harness` on every run. So the program's listeners see at most `cap` error events;
// past that they are not called, which ends the loop, and the record counts the events they did not see (`errorEventsWithheld`). What
// ran before the cut is recorded as it ran; a page under the cap is not touched.

/** Error events the program's listeners see in one run. The census's loop raises one a turn; no other program raised more than a few. */
export const ERROR_EVENT_CAP = 100;

/**
 * `wrapFor(listener)`: the listener to register in its place (`own`, the harness's listener, as it is). `wrapped` maps a listener to its
 * wrapper (removeEventListener). `state.withheld`: the events past the cap.
 */
export function errorListeners({ own = null, onThrow = () => {}, cap = ERROR_EVENT_CAP } = {}) {
  const wrapped = new WeakMap();
  const state = { seen: 0, withheld: 0 };
  let last = null;
  // One count an event, however many listeners it reaches, so each of them sees the same events.
  const admit = (event) => {
    if (event !== last) {
      last = event;
      state.seen += 1;
      if (state.seen > cap) state.withheld += 1;
    }
    return state.seen <= cap;
  };
  const wrapFor = (listener) => {
    if (!listener || listener === own) return listener;
    let wrap = wrapped.get(listener);
    if (!wrap) {
      wrap = function (event) {
        if (!admit(event)) return undefined;
        try { return typeof listener === "function" ? listener.call(this, event) : listener.handleEvent(event); }
        catch (e) { onThrow(e); throw e; }
      };
      wrapped.set(listener, wrap);
    }
    return wrap;
  };
  return { wrapFor, wrapped, state };
}
