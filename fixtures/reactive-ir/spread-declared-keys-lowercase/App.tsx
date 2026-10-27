type Attrs = { class?: string; onclick?: () => void };

// On web rc.17 only `on` followed by an uppercase letter installs a listener
// (`assignProp`), so `onclick` is an attribute and the spread holds no
// handler: nothing reports. The rc.13 twin reads the same key as a listener.
export function Card(props: Attrs) {
  return <div {...props} />;
}
