// kui feature-messages MessagesRoute.tsx:444-446 (child at MessagesTab): a child spreads an object holding a closure that calls the callback prop onto
// a component, `{...(props.onApply === undefined ? {} : { onApply: ... })}`; the callback reads a component prop.
type Preset = { name: string };

function Row(props: { name: string; onApply?: (preset: Preset) => void }) {
  return <button onClick={() => props.onApply?.({ name: props.name })}>{props.name}</button>;
}

function Presets(props: { onApply?: (preset: Preset) => void }) {
  return (
    <Row
      name="a"
      {...(props.onApply === undefined ? {} : { onApply: (preset: Preset) => props.onApply?.(preset) })}
    />
  );
}

export function Route(props: { clusterId: string }) {
  return <Presets onApply={(preset) => console.log(props.clusterId, preset.name)} />;
}
