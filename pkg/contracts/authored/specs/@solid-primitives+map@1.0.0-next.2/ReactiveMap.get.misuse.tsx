import { createTrackedEffect } from "solid-js";
import { ReactiveMap } from "@solid-primitives/map";

export default function App() {
  const collection = new ReactiveMap<number, string>([[1, "one"]]);
  createTrackedEffect(() => { console.log(collection.get(1)); });
  return <p>leaf</p>;
}
