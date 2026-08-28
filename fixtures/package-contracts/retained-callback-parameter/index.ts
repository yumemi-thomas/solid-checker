interface Node {
  fn: () => void;
  value: number;
}

interface Slot {
  slot: () => void;
}

// Stores the callback instead of calling it. Nothing here proves the value is
// never invoked -- whoever receives `node` may invoke `node.fn` -- so the
// forwarding caller below cannot inherit an empty callback summary from this.
function createNode(fn: () => void, value: number): Node {
  const node = {
    fn,
    value
  };
  return node;
}

let retainedLocally: (() => void) | null = null;

// Positive: forwards a caller-supplied callback into a local helper that
// retains it. The transitive summary is empty, and an empty summary is the
// negative claim "never invoked", so the domain must be `unknown` instead.
export function forwardsIntoRetainingHelper(fn: () => void): Node {
  return createNode(fn, 1);
}

// Positive: retains the callback directly, in a module binding.
export function retainsInModuleBinding(fn: () => void): void {
  retainedLocally = fn;
}

// Positive: a rest parameter absorbs an unbounded argument tail that no
// `callbacks` row can name, so use of one of its elements leaves the exact
// callback claim locally open.
export function absorbsRest(...handlers: Array<() => void>): number {
  const first = handlers[0];
  return first ? 1 : 0;
}

// Negative: invocation is proven, so the row is stated and no sentinel appears.
export function invokesCallback(fn: () => void): void {
  fn();
}

// Negative: every reference observes the value without invoking or retaining
// it. The honest answer is the omitted (negative) claim, not a sentinel.
export function observesCallback(fn: () => void): boolean {
  return typeof fn === "function" && fn !== observesCallback;
}

// Negative: the container is the *caller's* own parameter. This function does
// not invoke what it wrote there, and the caller's code is analyzed too.
export function storesIntoCallerContainer(target: Slot, fn: () => void): void {
  target.slot = fn;
}
