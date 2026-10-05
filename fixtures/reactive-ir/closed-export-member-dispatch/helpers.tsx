// Exported helpers that invoke a member of a caller-supplied value. Which
// implementation runs belongs to each call site.

// Entered only through call expressions.
export function describe(item: { label(): string }) {
  return item.label();
}

// Also handed out as a value below, so something unseen may call it.
export function escapes(item: { label(): string }) {
  return item.label();
}

export const kept = [escapes];

// Entered through JSX, where no call site selects the member.
export function Card(props: { format: { label(): string } }) {
  const text = props.format.label();
  return <span>{text}</span>;
}
