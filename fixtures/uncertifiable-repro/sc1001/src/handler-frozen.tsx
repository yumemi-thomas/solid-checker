// inferay WorkspaceDockHandle/index.tsx:6,19: an exported component installs a prop as a DOM event listener once.
export function WorkspaceDockHandle(props: { draggable?: boolean; onDragStart?: (event: PointerEvent) => void }) {
  return <span onPointerDown={props.onDragStart}>{props.draggable ? "drag" : ""}</span>;
}
