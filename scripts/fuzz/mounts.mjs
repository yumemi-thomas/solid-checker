// The recall mounts (misuse.mjs RECALL_MOUNTS): where an exported parent renders the misuse component besides the base mounts
// (`root`, `child`, `show`, `for`, print.mjs). Each is a real way an app mounts a component, and each runs the component's body
// when the parent renders it, or later (`show-later`, on the parent's button `pm<N>`, which the driving plan clicks first).
//
//   show-signal   <Show when={on()}>, a signal that is true          show-later   <Show when={on()}>, false until the parent's button
//   show-fallback <Show when={true} fallback={...}>                  show-keyed   <Show when={1} keyed>{(v) => <C />}</Show>
//   for-signal    <For each={items()}>, a signal of one row          for-item     a row that renders the child under its item
//   for-unkeyed   <For each={[1]} keyed={false}>                     for-fallback <For each={[1]} fallback={...}>
//   switch        <Switch><Match when={true}>                        repeat       <Repeat count={1}>
//   loading       <Loading fallback={0}>                             loading-on   <Loading fallback={0} on={on()}>
//   errored       <Errored fallback={...}>                           dynamic      `const D = dynamic(() => C)`, rendered <D />
//   portal        <Portal> (@solidjs/web), the child in document.body
//
// The forms follow the rc.17 typings (solid-js types/client/flow.d.ts, @solidjs/web types/index.d.ts): `<Dynamic>` is deprecated
// there, so the dynamic mount is `dynamic()`.
//
// The wrapper mounts (WRAP_MOUNTS, ASYNC_OUTSIDE_LOADING_BOUNDARY only: misuse.mjs `wrapMounts`) render the child through a user
// component whose body holds a `<Loading>` and the child's `props.children` (ADR 0668: the cover walk reads `children` inside it):
//
//   wrap-real     `props.children` inside the `<Loading>`: covered (legal)    wrap-alias    `const p = props; p.children`: covered (legal)
//   wrap-foreign  the children render outside; `other.children` is inside     wrap-outside  the same with `other.label` inside

export const RECALL_MOUNTS = ["show-signal", "show-later", "show-fallback", "show-keyed", "for-signal", "for-item", "for-unkeyed", "for-fallback",
  "switch", "repeat", "loading", "loading-on", "errored", "dynamic", "portal"];
/** The wrapper mounts: a user component around the child, with a `<Loading>` of its own (misuse.mjs `wrapMounts`, `recall.test.mjs`). */
export const WRAP_MOUNTS = ["wrap-real", "wrap-alias", "wrap-foreign", "wrap-outside"];
/** Mounts whose child renders only after the parent's button (`pm<N>`) is clicked. */
export const LATER_MOUNTS = ["show-later"];

const INDENT = "  ";
const indented = (lines) => lines.map((line) => `${INDENT}${line}`);

/**
 * The parent's lines for a recall mount, or null for a base mount (print.mjs prints those).
 * @param {string} mount
 * @param {string[]} wrapped the child's JSX lines (`<C1 />`, or wrapped in a provider by a source)
 * @param {{ index: number, name: string, used: Set<string>, web: Set<string> }} at the parent's number, the child's name, and the
 *   import sets (`solid-js`, `@solidjs/web`) the printer fills
 * @returns {{ lines: string[], prelude: string[], module?: string[] } | null} `module`: declarations at module scope, above the parent
 */
export function mountParts(mount, wrapped, { index, name, used, web }) {
  const on = `on${index}`;
  const child = wrapped.join("");
  switch (mount) {
    case "show-signal": used.add("Show").add("createSignal"); return { prelude: [`const [${on}] = createSignal(true);`], lines: [`<Show when={${on}()}>`, ...indented(wrapped), "</Show>"] };
    case "show-later": {
      used.add("Show").add("createSignal");
      const setter = `setOn${index}`;
      return { prelude: [`const [${on}, ${setter}] = createSignal(false);`],
        lines: [`<button data-act="pm${index}" onClick={() => ${setter}(true)}>mount</button>`, `<Show when={${on}()}>`, ...indented(wrapped), "</Show>"] };
    }
    case "show-fallback": used.add("Show"); return { prelude: [], lines: ["<Show when={true} fallback={<p>f</p>}>", ...indented(wrapped), "</Show>"] };
    case "show-keyed": used.add("Show"); return { prelude: [], lines: ["<Show when={1} keyed>", `${INDENT}{(v: number) => ${child}}`, "</Show>"] };
    case "for-signal": used.add("For").add("createSignal"); return { prelude: [`const [items${index}] = createSignal([1]);`], lines: [`<For each={items${index}()}>`, `${INDENT}{() => ${child}}`, "</For>"] };
    case "for-item": used.add("For").add("Show"); return { prelude: [], lines: ["<For each={[1]}>", `${INDENT}{(item) => <Show when={item}>${child}</Show>}`, "</For>"] };
    case "for-unkeyed": used.add("For"); return { prelude: [], lines: ["<For each={[1]} keyed={false}>", `${INDENT}{() => ${child}}`, "</For>"] };
    case "for-fallback": used.add("For"); return { prelude: [], lines: ["<For each={[1]} fallback={<p>f</p>}>", `${INDENT}{() => ${child}}`, "</For>"] };
    case "switch": used.add("Switch").add("Match"); return { prelude: [], lines: ["<Switch>", `${INDENT}<Match when={true}>`, ...indented(indented(wrapped)), `${INDENT}</Match>`, "</Switch>"] };
    case "repeat": used.add("Repeat"); return { prelude: [], lines: ["<Repeat count={1}>", ...indented(wrapped), "</Repeat>"] };
    case "loading": used.add("Loading"); return { prelude: [], lines: ["<Loading fallback={0}>", ...indented(wrapped), "</Loading>"] };
    case "loading-on": used.add("Loading").add("createSignal"); return { prelude: [`const [${on}] = createSignal(1);`], lines: [`<Loading fallback={0} on={${on}()}>`, ...indented(wrapped), "</Loading>"] };
    case "errored": used.add("Errored"); return { prelude: [], lines: ["<Errored fallback={<p>e</p>}>", ...indented(wrapped), "</Errored>"] };
    case "dynamic": web.add("dynamic"); return { prelude: [`const D${index} = dynamic(() => ${name});`], lines: [`<D${index} />`] };
    case "portal": web.add("Portal"); return { prelude: [], lines: ["<Portal>", ...indented(wrapped), "</Portal>"] };
    case "wrap-real": case "wrap-alias": case "wrap-foreign": case "wrap-outside": return wrappedBy(mount, wrapped, index, used);
    default: return null;
  }
}

/** The wrapper mounts' parts: the wrapper `Wrap<N>` at module scope and the parent's lines around the child. */
function wrappedBy(mount, wrapped, index, used) {
  used.add("Loading");
  const wrapper = `Wrap${index}`;
  const holding = (children) => ["<Loading fallback={0}>", ...indented(children), "</Loading>"];
  const body = {
    "wrap-real": { before: [], view: holding(["{props.children}"]) },
    "wrap-alias": { before: ["const p = props;"], view: holding(["{p.children}"]) },
    // The real children render outside the boundary; the `<Loading>` holds the `children` of another object (ADR 0668).
    "wrap-foreign": { before: ["const other = { children: 1 };"], view: ["<div>", ...indented(["{props.children}", ...holding(["<span>{other.children}</span>"])]), "</div>"] },
    "wrap-outside": { before: ["const other = { label: 1 };"], view: ["<div>", ...indented(["{props.children}", ...holding(["<span>{other.label}</span>"])]), "</div>"] },
  }[mount];
  return {
    prelude: [],
    module: [`function ${wrapper}(props: { children: any }) {`, ...indented(body.before), `${INDENT}return (`, ...indented(indented(body.view)), `${INDENT});`, "}", ""],
    lines: [`<${wrapper}>`, ...indented(wrapped), `</${wrapper}>`],
  };
}
