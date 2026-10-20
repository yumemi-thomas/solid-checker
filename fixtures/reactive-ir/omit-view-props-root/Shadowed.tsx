// A local `omit` that is not Solid's: it returns its argument, so the "view"
// is the props object itself and a write through it would reach the props.
function omit<T extends object>(props: T, ..._keys: string[]): T {
  return props;
}

export function ShadowedOmit(props: { value: string }) {
  const same = omit(props, "class");
  same.value = "reset";
  const v = props.value;
  return <span>{v}</span>;
}
