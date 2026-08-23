// The node build returns a plain object. It proves *no* reactive return, which
// is a certified negative and not an absence of knowledge.
export function Show(props) {
  return { view: props.when };
}

export function Steady() {
  return { view: 1 };
}
