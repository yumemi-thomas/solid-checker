import { getOwner, onCleanup } from "solid-js";

const f = () => {};
declare function xs(): AsyncIterable<number>;

// Past a suspension of the guarded block, the owner the probe saw is gone.
export async function afterAwait() {
  if (getOwner()) {
    await Promise.resolve();
    onCleanup(f);
  }
}
export async function forAwait() {
  if (getOwner()) {
    for await (const x of xs()) {
      void x;
      onCleanup(f);
    }
  }
}
export async function laterIteration() {
  if (getOwner()) {
    for (let i = 0; i < 2; i += 1) {
      onCleanup(f);
      await Promise.resolve();
    }
  }
}
export async function logicalRight() {
  getOwner() && (await Promise.resolve(), onCleanup(f));
}

// The probe still holds: before the suspension, or the loop tests it again.
export async function before() {
  if (getOwner()) {
    onCleanup(f);
    await Promise.resolve();
  }
}
export async function probedEachPass() {
  for (let i = 0; i < 2; i += 1) {
    if (getOwner()) {
      onCleanup(f);
      await Promise.resolve();
    }
  }
}
export async function otherArm(flag: boolean) {
  if (getOwner()) {
    if (flag) {
      onCleanup(f);
    } else {
      await Promise.resolve();
    }
  }
}
