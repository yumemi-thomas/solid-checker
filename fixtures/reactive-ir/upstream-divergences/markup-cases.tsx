// v1/no-innerhtml with upstream's branch tree: a static value that is real
// markup passes (unless children also compete for the content), a static
// value that is not markup asks for innerText, a dynamic value is dangerous,
// and lowercase `innerhtml` is an ordinary attribute the rule ignores.
//
// Every case here is about `innerHTML` itself, which is a *declared* Solid prop
// — TypeScript accepts all of them, so each claim is the checker's own.
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

// Permissive props: TypeScript admits any key, so the React-name arm is the only
// thing that can speak here.
const Panel = (props: Record<string, unknown>) => <div>{String(props.id)}</div>;

// The React prop, whose arm was narrowed to components on 2026-08-17. On an
// intrinsic element `dangerouslySetInnerHTML` is not in `JSX.IntrinsicElements`,
// so TS2322 says "Property 'dangerouslySetInnerHTML' does not exist" — which is
// word for word the arm's own claim that the prop is not supported. On a
// component the prop is a permitted key, TypeScript is silent, and the claim
// (Solid's renderer has no special case for the name, so it arrives inert)
// stands alone. The rewrite fix rides the surviving half.
export function ReactMarkupProp(props: { html: string }) {
  return (
    <div>
      {/* Silent: TS2322 already says this. */}
      <div dangerouslySetInnerHTML={{ __html: props.html }} />
      {/* Reported, with the innerHTML rewrite. */}
      <Panel dangerouslySetInnerHTML={{ __html: props.html }} />
      {/* Reported without a fix: `__html` is not the only entry, so there is no
          unambiguous rewrite. */}
      <Panel dangerouslySetInnerHTML={{ __html: props.html, extra: 1 }} />
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
