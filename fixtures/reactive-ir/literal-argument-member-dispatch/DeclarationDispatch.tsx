function pageSlice(items: readonly number[]): number[] {
  return items.slice(0, 2);
}

declare function keep(value: unknown): void;

// Local function declarations have the same exact-caller proof as arrows.
export function LocalDeclaration(props: { items: readonly number[] }) {
  function visible() { return pageSlice(props.items); }
  return <div>{visible().length}</div>;
}

export function EscapedDeclaration(props: { items: readonly number[] }) {
  function visible() { return pageSlice(props.items); }
  keep(visible);
  return <div>{visible().length}</div>;
}
