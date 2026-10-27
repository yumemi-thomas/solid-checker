// app-game math-examples camera-projection-svg/cube.tsx:34-36, 50: `attr:data-z` is refused by the
// pinned compiler (SC9015, a violation), so no read in the file is placed.
import { createSignal, For } from "solid-js";

type Point = [number, number, number];

export function Cube() {
  const [vertex] = createSignal<Point[]>([]);
  return (
    <For keyed={false} each={vertex()}>
      {(point) => <circle cx={point()[0]} cy={point()[1]} attr:data-z={vertex()[0][2]} r=".02" />}
    </For>
  );
}
