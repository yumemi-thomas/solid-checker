// app-game solid-dnd-playground NestedDemo.tsx:87 (createDragSensor, solid-dnd createDragSensor.tsx:515-523): a callback that writes a
// signal sits in an options object handed to a project function whose options parameter has a default (`options = {}`) and which
// hands it to a method its factory's interface declares (`factory.createSensor(options)`).
import { createSignal } from "solid-js";

type Options = { onDrop?: () => void };

interface Factory {
  createSensor(options?: Options): number;
}

function createFactory(): Factory {
  const sessions: Options[] = [];
  return { createSensor: (options = {}) => sessions.push(options) };
}

function createDragSensor(options: Options = {}) {
  return createFactory().createSensor(options);
}

export function Board() {
  const [dropped, setDropped] = createSignal(0);
  createDragSensor({ onDrop: () => setDropped(dropped() + 1) });
  return <p>{dropped()}</p>;
}
