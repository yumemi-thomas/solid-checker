declare module "@solidjs/web/server-functions/rich-args" {
  export function enableRichArguments(): void;
}
declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: Element): () => void;
}
