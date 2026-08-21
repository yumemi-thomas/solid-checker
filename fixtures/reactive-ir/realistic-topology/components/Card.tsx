// A component in its own file, rendered from another file. Its prop backing is
// decided by those call sites, which is why cross-file topology is what makes
// this provable at all.
export function Card(props: { title: string }) {
  const title = props.title;
  return <h1>{title}</h1>;
}

// The same defect through a parameter destructure.
export function Badge({ label }: { label: string }) {
  return <span>{label}</span>;
}

// Rendered only with a static value: the prop compiles to a plain property and
// reading it once is correct.
export function Plaque(props: { note: string }) {
  const note = props.note;
  return <span>{note}</span>;
}
