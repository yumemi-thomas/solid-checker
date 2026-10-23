// Kept: `Main.ts` reads this module as a namespace and hands it out
// unnamed, so a caller of `Hidden` may exist that no reference shows.
export function Hidden(props: { label: string; onPick?: () => void }) {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
}
