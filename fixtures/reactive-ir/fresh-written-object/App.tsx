import { createSignal } from "solid-js";

// The shapes of `@solidjs/router` next.41's six open writes (ADR 1024): every one writes an object this file makes.
export function extractSearchParams(entries: [string, unknown][]) {
  const params = Object.create(null);
  entries.forEach(([key, value]) => {
    params[key] = value;
  });
  return { ...params };
}

export function createMatcher(keys: string[], values: unknown[]) {
  const match = { path: "", params: {} as Record<string, unknown> };
  for (let i = 0; i < keys.length; i++) {
    match.params[keys[i]] = values[i];
  }
  return match;
}

export function scrollPositions(stored: string, depth: number, value: unknown) {
  let positions: Record<number, unknown> = {};
  positions = JSON.parse(stored) || {};
  positions[depth] = value;
  return positions;
}

export function searchParams(raw: Record<string, unknown>, outcomes: unknown[]) {
  let result: Record<string, unknown> | undefined;
  for (const outcome of outcomes) {
    result = Object.assign(result || { ...raw }, outcome);
  }
  return result || raw;
}

export function hashKey(value: Record<string, unknown>) {
  return Object.keys(value)
    .sort()
    .reduce((result: Record<string, unknown>, key) => {
      result[key] = value[key];
      return result;
    }, Object.create(null));
}

// A standard root goes on as a value.
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
