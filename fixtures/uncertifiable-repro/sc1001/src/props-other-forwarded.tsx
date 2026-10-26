// app-game browser-atlas TreeView.tsx:154,175-184: a local component rendered by a tag whose prop is forwarded from
// the props of an exported component, whose own callers are unknown.
function CollapsedSummary(props: { label: string }) {
  const first = props.label;
  return <button>{first}</button>;
}

export function TreeView(props: { label: string }) {
  return <div><CollapsedSummary label={props.label} /></div>;
}
