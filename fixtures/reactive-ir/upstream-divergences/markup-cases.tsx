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

const MARKUP = "<b>hi</b>";

// "Static" is the *value* the compiler proves, not a literal type. A folded
// concatenation and a `const` reference are as static as the written literal
// above — TypeScript widens both to `string`, so recovering the value from a
// literal type reported all three as dangerous. Only the operand that is
// genuinely not constant still is.
export function FoldedMarkup(props: { html: string }) {
  return (
    <div>
      <div innerHTML={"<b>" + "hi</b>"} />
      <div innerHTML={MARKUP} />
      <div innerHTML={MARKUP + "<i>!</i>"} />
      <div innerHTML={props.html + "<i>!</i>"} />
    </div>
  );
}

// The same fact read by `v1/jsx-no-script-url`, where folding *adds* a
// finding: the scheme is proven whether it is written whole or assembled, and
// a value that is not constant stays uncertifiable rather than guessed.
export function ScriptUrls(props: { url: string }) {
  return (
    <div>
      <a href={"javascript:alert(1)"}>written</a>
      <a href={"java" + "script:alert(1)"}>folded</a>
      <a href={props.url}>dynamic</a>
    </div>
  );
}
