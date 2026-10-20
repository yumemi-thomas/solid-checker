// Runtime witnesses for the original ambient test inputs; not package contracts.
const noop = (..._args: unknown[]) => undefined;
const runner = { run: noop };
const values: Record<string, unknown> = {
  opaque: runner, partial: runner, makeOpaque: () => runner,
  opaqueFn: (value: { run(): void }) => value.run(),
  opaqueCb: (callback: () => void) => callback(),
  opaqueSlot: (callback: () => void) => callback(),
  sink: noop, c: true, p: Promise.resolve(1), src: runner,
  rest: {}, nothing: undefined, obj: runner, parse: JSON.parse,
  mix: (base: new () => object) => base,
};
for (const [key, value] of Object.entries(values)) Reflect.set(globalThis, key, value);
export {};
