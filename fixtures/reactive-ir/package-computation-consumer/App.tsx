import { startTicker, startTickerAlways, startTickerSilent } from "reactive-package";

// The claim under test. `startTicker`'s accepted contract states one `compute`
// in `computations` (ADR 0114): the call may register a computation on its
// caller's owner -- its count is `min: 0`. At module scope there is none, so
// the call is an owner proof obligation (ADR 0161): whether it registers at
// all is what the contract does not say.
startTicker();

// `startTickerAlways` states the same `compute` with `min: 1`: every call
// registers. At module scope that is a proven violation, as a direct
// `createEffect` here would be.
startTickerAlways();

// Inside a component the caller's owner exists, and the same calls owe nothing.
export function Ticker() {
  startTicker();
  startTickerAlways();
  return <div>ticking</div>;
}

// The control: the same declaration and the same unowned call, with `creates`
// closed and no `computations` item. The consumer reads "no owner requirement",
// so the findings above rest on the item and on nothing else.
startTickerSilent();
