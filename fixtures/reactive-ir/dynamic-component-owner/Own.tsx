// A project component whose prop is spelled `component`. Nothing says what it does with the value.
export function Own(props: { component: () => unknown }) {
  return <section>{props.component()}</section>;
}
