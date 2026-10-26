// Clean. Mirrors helge-dev src/components/NavBar.tsx:15: the compute phase tracks, the apply phase
// writes; the same read in the apply is SC1001 (read-in-effect-apply).
import { createEffect, createSignal } from "solid-js";

function Nav(props: { path: string }) {
  const [opened, setOpened] = createSignal(true);
  createEffect(
    () => props.path,
    () => {
      setOpened(false);
    },
  );
  return <p>{opened() ? "open" : "closed"}</p>;
}

export function App() {
  const [path] = createSignal("/");
  return <Nav path={path()} />;
}
