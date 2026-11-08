import { getOwner, onCleanup } from "solid-js";

const f = () => {};

// Past a `yield`, the body runs on the next `.next()`: here a timer's, with no
// owner. rc.21 raises NO_OWNER_CLEANUP (Chrome); no edge follows the step.
export function LaterStep() {
  function* gen() {
    if (getOwner()) {
      yield 1;
      onCleanup(f);
    }
  }
  const it = gen();
  it.next();
  setTimeout(() => it.next(), 0);
  return <i />;
}

// Before the first `yield`, stepped in the body: on the body's stack, owned.
export function FirstStep() {
  function* gen() {
    onCleanup(f);
    yield 1;
  }
  gen().next();
  return <i />;
}

// Reviewer R60: a helper written outside the generator and called past its
// `yield` runs on the timer's step with no owner (NO_OWNER_CLEANUP in Chrome);
// the call's edge carries that, so the helper's own site is uncertifiable,
// also where the body calls it too.
function helper() {
  onCleanup(f);
}
export function HelperLaterStep() {
  function* gen() {
    yield 1;
    helper();
  }
  const it = gen();
  it.next();
  setTimeout(() => it.next(), 0);
  return <i />;
}
function helperAlsoInBody() {
  onCleanup(f);
}
export function HelperAlsoInBody() {
  helperAlsoInBody();
  function* gen() {
    yield 1;
    helperAlsoInBody();
  }
  const it = gen();
  it.next();
  setTimeout(() => it.next(), 0);
  return <i />;
}
