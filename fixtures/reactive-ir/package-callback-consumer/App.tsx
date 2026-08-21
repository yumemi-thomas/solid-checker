import { createEffect, createSignal } from "solid-js";
import {
  runDeferred,
  runLeaf,
  runMixed,
  runOwnedEffect,
  runTracked
} from "reactive-package";

const [count] = createSignal(0);

runOwnedEffect();

function readCount() {
  return count();
}

function effectInCallback() {
  createEffect(readCount, () => {});
}

export function Good() {
  runTracked(readCount);
  runDeferred(readCount);
  return <div>good</div>;
}

export function Bad() {
  runMixed(readCount);
  return <div>bad</div>;
}

export function Leaf() {
  runLeaf(effectInCallback);
  return <div>leaf</div>;
}

export function ExternalOwnerBad() {
  runOwnedEffect();
  return <div>owner</div>;
}
