import { createEffect, createMemo, createSignal } from "solid-js";
import * as helpers from "./helpers";

const [count] = createSignal(0);
const pause = () => Promise.resolve();

// Batch 11, slice 9B (ADR 0973): every read and every call of a project function written after an async function's own suspension ends in a row of SC1002, an SC9012, the
// compiler's discard or a typed reason, or it gets SC1002's uncertifiable row (blocker `read-after-await-unaccounted`): never a violation, never a silence. The producers
// walk only from a tracked async computation, so each site below was silent before the universe: nothing walks it and nothing said why.
//
// This program holds a tracked async computation with something after a suspension (`Tracked`), and no site below is that computation's own.
export function Tracked() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(
    () => m(),
    () => {},
  );
  return <div />;
}

// A read after an await in an async function no computation runs: an event handler (a role silence is left out on purpose, so the site keeps its row) and a helper.
export function Handler() {
  return (
    <button
      onClick={async () => {
        await pause();
        count();
      }}
    >
      go
    </button>
  );
}
export async function helper() {
  await pause();
  return count();
}

// A call of a project function after an await whose closure is not silent: it calls the standard library, reads a source, or calls itself.
function stringify(n: number) {
  return JSON.stringify({ n });
}
function readsCount() {
  return count();
}
function ping(n: number): number {
  return n > 0 ? pong(n - 1) : 0;
}
function pong(n: number): number {
  return ping(n);
}
export async function Calls() {
  await pause();
  stringify(1);
  readsCount();
  ping(3);
}

// The same through a namespace import: the callee is another module's function, resolved exactly, whose closure calls the standard library.
export async function Namespaced() {
  await pause();
  helpers.format(1);
}

// A call of a function value after an await (a parameter), and a call of a function that is not exactly resolved (a reassigned `let`).
export async function Invokes(callback: () => unknown) {
  await pause();
  callback();
}
let reassigned = () => 1;
reassigned = () => count();
export async function Reassigned() {
  await pause();
  reassigned();
}

// A site in a function a reviewed test runner schedules is a site like any other (owner ruling 9b): this one has no runner here, so it is a plain async function.
export async function Loop(items: number[]) {
  for await (const item of items) {
    count();
    void item;
  }
}

// A call of a project method after an await (owner ruling, slice 9B decision on members). A method of a `const` object literal nothing writes a member of is exact (ADR 0279):
// the call is a site and the callee summary applies; here the closure calls the standard library or reads a source, so the row stays.
const codec = {
  encode(n: number) {
    return JSON.stringify({ n });
  },
};
const meter = {
  read() {
    return count();
  },
};
export async function ByExactMember() {
  await pause();
  codec.encode(1);
  meter.read();
}

// A method an override or a replacement may stand in for is no exact callee: `this.helper()` in a class's own async method, and a receiver that is a parameter typed as the
// class. The call is a site whose row nothing ends, and no summary is asked of it.
class Loader {
  helper() {
    return 1;
  }
  async load() {
    await pause();
    return this.helper();
  }
}
export async function ByParameterReceiver(loader: Loader) {
  await pause();
  loader.helper();
}

