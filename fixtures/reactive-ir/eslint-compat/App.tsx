import { createSignal } from "solid-js";

const [name] = createSignal("");

// A `use:` directive must be in scope; `Right` applies it below.
const autofocus = (element: HTMLElement) => {
  element.focus();
};

// A component. Its props are collected into a plain object the compiler never
// lowers, so the duplicate-prop slot there is the key exactly as written.
const Panel = (props: Record<string, unknown>) => <div>{String(props.id)}</div>;

// The ESLint-compatible surface: six rule identities from
// eslint-plugin-solid 0.14.5, all decided by the parse alone. Each pair below
// is the same markup written wrong and written right, so a rule that stops
// firing and a rule that starts firing on correct code both move this
// snapshot.
export function Wrong() {
  return (
    <div>
      {/* SC8003: the second class wins and the first is dead */}
      <div class="card" id="a" class="wide" />
      {/* SC8003: click is delegated, so both occurrences lower to the
          property write `el.$$click = handler` and the later assignment
          wins — the first handler is dead */}
      <button onClick={() => {}} onClick={() => {}} />
      {/* SC8003 once, SC8001 twice: statically string-valued `on*` props are
          frozen into the template (each drawing the event-handlers
          attribute warning), where the HTML parser keeps the first `onfoo`
          and the second is dead */}
      <div onFoo="a" onFoo="b" />
      {/* SC8003: `0x10` and `0x20` are NumericLiteral nodes, which the
          compiler inlines into the template (as 16 and 32) — the same
          first-wins attribute slot. The `on-foo` spelling keeps SC8001 out
          of the way: its third character is not alphabetic, so the
          event-handlers rule does not look at it. */}
      <div on-foo={0x10} on-foo={0x20} />
      {/* SC8003 twice: a component's props are a plain object, so a repeated
          key overwrites whatever the DOM lowering would have said about the
          spelling. The namespaced pair additionally draws SC8012 twice —
          namespaces reach no compiler special case on a component. */}
      <Panel onSave={() => {}} onSave={() => {}} />
      <Panel on:click={() => {}} on:click={() => {}} />
      {/* SC8004: eval with a different spelling */}
      <a href="javascript:void(0)">dismiss</a>
      {/* SC8004 again: the browser decodes &#9; to a tab before URL parsing,
          so a character reference does not disarm it */}
      <a href="java&#9;script:alert(1)">tricky</a>
      {/* SC8008: parses as HTML and overwrites the child */}
      <div innerHTML={"<b>x</b>"}>overwritten</div>
      {/* SC8011: three React names Solid does not forward */}
      <label className="field" htmlFor="name" key="k">Name</label>
      {/* SC8012: not a Solid namespace, so not a directive */}
      <input model:value={name()} />
      {/* SC8016: childless, and multiline whitespace is not a child */}
      <span class="spacer"></span>
      <span>
      </span>
    </div>
  );
}

export function Right() {
  return (
    <div>
      <div class="card wide" id="a" />
      <a href="/dismiss">dismiss</a>
      <div>{name()}</div>
      <label class="field" for="name">Name</label>
      {/* The namespaces Solid's compiler recognises. */}
      <input
        prop:value={name()}
        attr:data-x="1"
        bool:disabled={true}
        on:click={() => {}}
        use:autofocus={true}
      />
      {/* Clean under the compiler's own lowering, though upstream folds all
          of these onto one name and calls them duplicates: `on:click` binds a
          bubble listener and `oncapture:click` a capture listener (both
          fire); a delegated `onClick` writes `$$click` while `on:click`
          attaches a listener (both fire); and `mouseenter` is not delegated,
          so each occurrence attaches its own listener (both fire). */}
      <button on:click={() => {}} oncapture:click={() => {}} />
      <button onClick={() => {}} on:click={() => {}} />
      <button onMouseEnter={() => {}} onMouseEnter={() => {}} />
      {/* `-1` parses as a UnaryExpression, not a NumericLiteral, so the
          compiler never freezes it into the template: each occurrence
          attaches its own listener and neither is dead. */}
      <div on-foo={-1} on-foo={-2} />
      {/* Two distinct keys on a props object — the component boundary has no
          case folding to merge them. */}
      <Panel onClick={() => {}} onclick={() => {}} />
      <span class="spacer" />
      <span>{name()}</span>
    </div>
  );
}
