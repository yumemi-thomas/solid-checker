// Kept: `Pages.ts` loads modules through `import.meta.glob`, which no
// reference shows, so `Page` may be rendered.
export function Page(props: { label: string; onPick?: () => void }) {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
}
