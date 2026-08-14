type Handle = { reset(): void };

export function Example(props: { ref: (handle: Handle) => void }) {
  props.ref({ reset() {} });
  return <div />;
}
