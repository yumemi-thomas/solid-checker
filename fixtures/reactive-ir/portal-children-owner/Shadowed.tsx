import { createMemo } from "solid-js";

// A project component named `Portal` is no `Portal` of the dialect: its children are a getter its own render reads, which nothing here proves. Still the uncertifiable rows.
function Portal(props: { children?: unknown }) {
  return <section>{props.children}</section>;
}

export function Shadowed() {
  const text = createMemo(() => "shadowed");
  return (
    <Portal>
      <div role="tooltip">{text()}</div>
    </Portal>
  );
}
