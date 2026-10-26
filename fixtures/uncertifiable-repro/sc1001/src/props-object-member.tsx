// app-game solid-dnd createDragSensor.tsx:552: a private component referenced from an object (`{ component: Page }`) is not rendered by a JSX tag.
import { createSignal } from "solid-js";

function Page(props: { title: string }) {
  const [shown, setShown] = createSignal(props.title);
  return <h1 onClick={() => setShown("x")}>{shown()}</h1>;
}

export const routes = [{ path: "/", component: Page }];
