// The shared preferences retain upstream's JSX-child and expensive-branch
// structural gates, but these static inputs stay clean after the checker adds
// its stricter reactive-governing-input requirement.
const items = ["a", "b"];

function Up() {
  return <span />;
}

function Down() {
  return <span />;
}

export function Lists() {
  return (
    <div>
      <ol>{items.map((item) => item.toUpperCase())}</ol>
      <ol>{items.map((item, index) => item + index)}</ol>
    </div>
  );
}

export function NotRendered() {
  const rows = items.map((item) => <li>{item}</li>);
  return <ol>{rows}</ol>;
}

export function ShowCases(props: { open: boolean }) {
  return (
    <div>
      {props.open && <span />}
      <button icon={props.open ? <Up /> : <Down />} />
    </div>
  );
}
