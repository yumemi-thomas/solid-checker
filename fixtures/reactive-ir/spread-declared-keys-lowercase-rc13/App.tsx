type Attrs = { class?: string; onclick?: () => void };

// On web rc.13 any `on`-prefixed spread key installs a listener, so the
// declared `onclick` key is a possible handler and the spread keeps its
// obligation. The rc.17 twin reads the same key as an attribute.
export function Card(props: Attrs) {
  return <div {...props} />;
}
