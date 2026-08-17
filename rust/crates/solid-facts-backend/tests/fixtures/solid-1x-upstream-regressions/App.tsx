const handler = "plain attribute";
const html = "<b>content</b>";

// A parameter with the same spelling as another binding must resolve to the
// parameter, not to the first declaration in file order.
export function ShadowedHandler(handler: () => void) {
  return <button onClick={handler}>run</button>;
}

// JavaScript escape sequences are part of the literal's runtime value.
export function EscapedScriptUrl() {
  return <a href={'\u006aavascript:alert(1)'}>unsafe</a>;
}

// JSX attribute text does not interpret JavaScript backslash escapes.
export function JsxBackslashIsLiteral() {
  return <a href="\u006aavascript:alert(1)">literal backslash</a>;
}

// The static-value folder must terminate on cyclic declarations.
// @ts-ignore: the cycle is deliberate regression input.
const cyclicUrl: string = cyclicUrl;
export function CyclicUrl() {
  return <a href={cyclicUrl}>cycle</a>;
}

// The `dangerouslySetInnerHTML` arm was narrowed to components on 2026-08-17:
// on an intrinsic element the prop is TS2322 ("Property
// 'dangerouslySetInnerHTML' does not exist"), which is the arm's own claim. The
// `@ts-expect-error` that used to sit here was the tell -- the fixture was
// asserting a rule on code TypeScript rejects.
//
// The fix shapes are what this case is for, so they move to a component, whose
// props admit the key and where TypeScript is silent.
const Panel = (props: Record<string, unknown>) => <div>{String(props.id)}</div>;

export function InnerHtmlFixes() {
  return (
    <>
      <Panel dangerouslySetInnerHTML={{ __html: html }} />
      {/* @ts-expect-error: object addition is deliberate regression input. */}
      <Panel dangerouslySetInnerHTML={({ __html: html }) + ({})} />
      {/* Negative: on an intrinsic element this is TypeScript's to report. */}
      <div dangerouslySetInnerHTML={{ __html: html }} />
    </>
  );
}
