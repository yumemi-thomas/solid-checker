// probus-hk RollingNumber.tsx:194 (tag at :74): a local component rendered by a tag whose prop is forwarded from the
// props of an exported component, whose own callers are unknown.
function CollapsedSummary(props: { label: string }) {
  const first = props.label;
  return <button>{first}</button>;
}

export function TreeView(props: { label: string }) {
  return <div><CollapsedSummary label={props.label} /></div>;
}
