// kui NavItem.tsx:74: a component referenced from a route table (`{ component: Page }`) is not rendered by a JSX tag.
import { createSignal } from "solid-js";

function Page(props: { title: string }) {
  const [shown, setShown] = createSignal(props.title);
  return <h1 onClick={() => setShown("x")}>{shown()}</h1>;
}

export const routes = [{ path: "/", component: Page }];
