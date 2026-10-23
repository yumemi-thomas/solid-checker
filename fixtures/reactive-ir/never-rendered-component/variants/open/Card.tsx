// Kept: this package publishes `main`, so the program is open (ADR 0193)
// and a caller outside it may render `UnusedExported`.
export function UnusedExported(props: { label: string; onPick?: () => void }) {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
}
