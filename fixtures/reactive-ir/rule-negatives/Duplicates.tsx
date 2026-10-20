// jsx-no-duplicate-props (SC8003): one content mechanism per element; a
// component prop named innerHTML is an ordinary key.
function Widget(props: { innerHTML: string; children?: unknown }) {
  return <div>{props.children}{props.innerHTML}</div>;
}
export function Content(props: { markup: string; fallback: string }) {
  return (
    <div>
      <div>{props.fallback}</div>
      <Widget innerHTML={props.markup}>{props.fallback}</Widget>
    </div>
  );
}
