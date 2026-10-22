// `input` is not in the shared stub: declaration merging adds it here.
declare namespace JSX {
  interface IntrinsicElements {
    input: { required?: boolean; onInvalid?: (event: Event) => void };
  }
}
