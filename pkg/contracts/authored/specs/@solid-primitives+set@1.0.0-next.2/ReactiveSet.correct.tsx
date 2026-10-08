import { createTrackedEffect } from "solid-js";
import { ReactiveSet } from "@solid-primitives/set";

export default function App() {
  const collection = new ReactiveSet<number>([1]);
  return <p>{String(collection.has(1))}</p>;
}
