import { createSignal } from "solid-js";
import { callHandler, callHandlerOpen, passThrough, readKey } from "reactive-package";

const [count] = createSignal(0);
const event = { defaultPrevented: false };

function noop() {}

// `callHandler`'s accepted contract closes `returns` over two returns: the
// event's `defaultPrevented`, a member of argument 0 read at return time, and
// the `undefined` its optional chain hands back for a nullish event. The
// import finds nothing open, and using the result asks nothing more of the
// package.
export function Handler() {
  const prevented = callHandler(event, noop);
  return <div>{prevented ? "prevented" : "open"}</div>;
}

// The control: the same declaration and the same use, with `returns` left
// open, so the import reports the open claim. The closure above has to remove
// exactly this finding.
export function HandlerOpen() {
  const prevented = callHandlerOpen(event, noop);
  return <div>{prevented ? "prevented" : "open"}</div>;
}

// The reference: `passThrough`'s contract closes `returns` over argument 0
// itself (ADR 0075), which the consumer resolves exactly, so calling what it
// returns is the untracked read of `count` it is.
export function PassThrough() {
  passThrough(count)();
  return <div />;
}

// The stated limit, in the hiding direction. `readKey`'s contract closes
// `returns` over argument 0's `key`, and the literal here holds `count`
// there, so the call does return the accessor, and calling it is the same
// untracked read `PassThrough` makes. The consumer does not resolve the
// member: it is whatever the argument holds at return time, which code the
// call runs before then may have replaced, so the literal at the call does not
// determine it. The return reads as no reactive return, and it is never read
// as the argument itself either.
export function MemberRead() {
  readKey({ key: count })();
  return <div />;
}
