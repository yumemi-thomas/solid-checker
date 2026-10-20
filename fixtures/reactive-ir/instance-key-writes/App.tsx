type Runner = { run: () => void };

function helper(o: Runner) {
  o.run();
}

// The constructor registers every instance; `put` writes a member nothing
// names into each. An assignment through a non-literal key on a receiver
// that is not a proven built-in value copies keys into objects, so no
// project instance's `run` is trusted in this program.
const registry: Record<string, unknown>[] = [];
function put(key: string, value: unknown) {
  for (const target of registry) target[key] = value;
}
class Box {
  constructor() {
    registry.push(this as unknown as Record<string, unknown>);
  }
  run() {}
}
export function KeyWrite() {
  const box = new Box();
  put("run", opaque.run);
  helper(box);
  return <main />;
}
