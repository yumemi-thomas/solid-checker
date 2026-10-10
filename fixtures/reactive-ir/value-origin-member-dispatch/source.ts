import { createSignal } from "solid-js";

const [count] = createSignal(0);

export function evens(list: number[]): number[] {
  return list.filter((value) => value % 2 === 0);
}

export function stamp(date: Date): number {
  return date.getTime();
}

export function contains(set: Set<string>, key: string): boolean {
  return set.has(key);
}

export class Counter {
  total = 0;
  bump(): number {
    this.total += 1;
    return this.total;
  }
}

export class Ticker extends Counter {}

export class Live {
  now(): number {
    return count();
  }
}

export class Patched {
  run(): number {
    return 1;
  }
}

export function bump(counter: Counter): number {
  return counter.bump();
}

export function peek(live: Live): number {
  return live.now();
}

export function run(patched: Patched): number {
  return patched.run();
}

const patched = new Patched();
patched.run = () => count();

// A named write through a prototype replaces only the member it names.
(Live.prototype as unknown as { extra?: () => number }).extra = () => 0;
