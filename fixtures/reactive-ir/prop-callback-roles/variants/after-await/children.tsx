type Run = { run: () => Promise<void> };

// The prop is called in the component's body: an owned scope.
export function Child(props: Run) {
  props.run();
  return <main />;
}
