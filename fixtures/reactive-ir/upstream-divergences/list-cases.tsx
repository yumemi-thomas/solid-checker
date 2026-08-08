// v1/prefer-for and v1/prefer-show fire on JSX *child* position, exactly as
// upstream's container checks do: a `.map()` rendered as children reports
// whether or not the callback builds JSX, a `.map()` assigned to a variable
// does not, and a conditional inside an attribute value stays silent.
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
