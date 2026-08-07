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

export function InnerHtmlFixes() {
  return (
    <>
      <div dangerouslySetInnerHTML={{ __html: html }} />
      {/* @ts-expect-error: object addition is deliberate regression input. */}
      <div dangerouslySetInnerHTML={({ __html: html }) + ({})} />
    </>
  );
}
