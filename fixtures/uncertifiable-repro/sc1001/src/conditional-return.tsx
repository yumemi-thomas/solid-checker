// ai-memory-ui users.tsx:469-485: an exported component picks what it returns from a prop, in a body that runs once.
export function StatusCell(props: { disabled: boolean }) {
  if (props.disabled) {
    return <b>disabled</b>;
  }
  return <i>active</i>;
}
