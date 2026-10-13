import { createTrackedEffect } from "solid-js";
import { ReactiveSet } from "@solid-primitives/set";

export default function App() {
  const collection = new ReactiveSet<number>([1]);
  createTrackedEffect(() => { console.log(collection.values().next()); });
  return <p>leaf</p>;
}
