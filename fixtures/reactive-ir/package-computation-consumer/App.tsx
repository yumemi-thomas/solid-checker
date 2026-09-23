import { startTicker, startTickerSilent } from "reactive-package";

// The claim under test. `startTicker`'s accepted contract states one `compute`
// in `computations` (ADR 0114): the call registers a computation on its
// caller's owner. At module scope there is none, so the call owes the owner a
// direct `createEffect` here would.
startTicker();

// Inside a component the caller's owner exists, and the same call owes nothing.
export function Ticker() {
  startTicker();
  return <div>ticking</div>;
}

// The control: the same declaration and the same unowned call, with `creates`
// closed and no `computations` item. The consumer reads "no owner requirement",
// so the finding above rests on the item and on nothing else.
startTickerSilent();
