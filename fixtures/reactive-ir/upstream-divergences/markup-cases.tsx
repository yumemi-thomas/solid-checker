// v1/no-innerhtml with upstream's branch tree: a static value that is real
// markup passes (unless children also compete for the content), a static
// value that is not markup asks for innerText, a dynamic value is dangerous,
// and lowercase `innerhtml` is an ordinary attribute the rule ignores.
export function Markup(props: { html: string }) {
  return (
    <div>
      <div innerHTML={"<b>hi</b>"} />
      <div innerHTML={"a < b > c"} />
      <div innerHTML={"<b>hi</b>"}>
        <span />
      </div>
      <div innerhtml={props.html} />
      <div innerHTML={props.html} />
    </div>
  );
}
