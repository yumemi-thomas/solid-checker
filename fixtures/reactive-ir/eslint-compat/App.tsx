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
      {/* Silent since 2026-08-17: two byte-identical attribute names are
          TS17001 ("JSX elements cannot have multiple attributes with the same
          name"), so the duplicate-prop rule was narrowed off that spelling —
          on intrinsics and components alike (docs/precision-backlog.md). */}
      <div class="card" id="a" class="wide" />
      <button onClick={() => {}} onClick={() => {}} />
      {/* SC8003, the surviving domain: two *differently spelled* props that the
          DOM lowering folds into one slot. `onClick` and `onclick` both become
          the delegated `el.$$click = handler` property write, so the later
          assignment wins and the first handler is dead — and TypeScript sees
          two distinct, legal properties and says nothing. */}
      <button onClick={() => {}} onclick={() => {}} />
      {/* SC8003 again: `attr:title` and `title` share the one static template
          attribute slot, where the first wins. Also distinctly spelled, so
          also invisible to TypeScript's duplicate checks. (`attr:` resolves
          through the user-augmentable `JSX.ExplicitAttributes`, empty by
          default, so a project reaching this markup has declared the name —
          the documented way to use `attr:`. TypeScript's complaint about an
          *undeclared* `attr:` name is a different claim from the slot one.) */}
      <div attr:title="first" title="second" />
      {/* SC8003: a spread followed by an attribute. The later attribute
          legitimately wins, so TypeScript reports nothing even though the two
          names are identical — the one identical-name order it leaves alone. */}
      <div {...{ id: "spread" }} id="attribute" />
      {/* Silent on both counts now. `onFoo` does not exist on
          `HTMLAttributes<HTMLDivElement>` (TS2322), which was the
          event-handlers rule's claim here, and the two occurrences are
          byte-identical names (TS17001), which was the duplicate-prop rule's.
          Kept as a negative case: it is the exact markup both narrowings were
          made for. */}
      <div onFoo="a" onFoo="b" />
      {/* Silent: Solid 1.x declares both lowercase spellings. The retired
          event-handlers rule treated canonical casing as a policy. */}
      <div onclick={() => {}} />
      <div ondblclick={() => {}} />
      {/* A hyphenated tag is TS2339 against stock typings, so a project using
          one has declared it — commonly permissively. There TypeScript is
          silent, but the retired event-handlers rule's remaining objections
          were conventions rather than proven runtime defects. */}
      <my-widget onFoo="a" />
      <my-widget onlynow={() => {}} />
      {/* Silent: `0x10` and `0x20` are NumericLiteral nodes the compiler
          inlines into the template (as 16 and 32), so they do share the
          first-wins attribute slot — but the two names are byte-identical, so
          TS17001 covers it. Retained because it is the only fixture case
          pinning that numeric literals reach the static-value branch at all. */}
      <div on-foo={0x10} on-foo={0x20} />
      {/* The component pairs are byte-identical too, so SC8003 is silent and
          TS17001 speaks. They stay because the namespaced pair still draws
          SC8012 twice — namespaces reach no compiler special case on a
          component, where props are a plain object. */}
      <Panel onSave={() => {}} onSave={() => {}} />
      <Panel on:click={() => {}} on:click={() => {}} />
      {/* SC8004: eval with a different spelling */}
      <a href="javascript:void(0)">dismiss</a>
      {/* SC8004 again: the browser decodes &#9; to a tab before URL parsing,
          so a character reference does not disarm it */}
      <a href="java&#9;script:alert(1)">tricky</a>
      {/* SC8008, and SC8003 for the content conflict: `innerHTML` and JSX
          children both write the element's content, and no type relates them —
          both are declared props with legal values, so TypeScript is silent. */}
      <div innerHTML={"<b>x</b>"}>overwritten</div>
      {/* SC8003 again, the other conflict no type describes. */}
      <div innerHTML="x" textContent="y" />
      {/* Silent since 2026-08-17: the `children`-prop-plus-JSX-children pair is
          TS2710 ("'children' are specified twice. The attribute named 'children'
          will be overwritten."), word for word the rule's claim — in both passes
          and on components too. Only that exact pair was narrowed away; a set
          that also includes innerHTML or textContent still reports, because the
          finding then asserts more than TS2710 does. */}
      <div children={<i />}>
        <span />
      </div>
      <Panel children={<i />}>
        <span />
      </Panel>
      {/* SC8011: three React names Solid does not forward */}
      <label className="field" htmlFor="name" key="k">Name</label>
      {/* Silent since 2026-08-17: on an intrinsic element an unrecognised
          namespace is TS2322 ("Property 'model:value' does not exist on type
          'InputHTMLAttributes<HTMLInputElement>'"), so SC8012 was narrowed to
          components — where props are a plain object and the claim (the
          compiler special-cases namespaces only on DOM elements it lowers, so
          the prop arrives inert) is one no type makes. The `<Panel on:click>`
          pair above is that surviving arm. */}
      <input model:value={name()} />
      {/* SC8012 twice, the other surviving arm and the boundary of the argument
          above: TypeScript does not type-check a JSX attribute name containing a
          *hyphen* at all — a deliberate exemption for HTML's own hyphenated
          attributes — so it declines to look here even though the element is
          intrinsic. A first, coarser narrowing lost these two; upstream's own
          cases 04 and 05 are this shape. */}
      <div class:mt-10={true} />
      <div class:mt-10 />
      {/* Silent: `onFoo-bar` carries a hyphen, so TypeScript does not check its
          name; the checker nevertheless has no proven defect to report. */}
      <div onFoo-bar="a" />
      {/* SC1005: a hyphenated attribute is the one native-attribute position
          that survived, for the same reason — the accessor is stringified in
          and no type objects. */}
      <div data-count={name} />
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
      {/* `-1` and `NaN` are primitive numbers to TypeScript, but neither is a
          StringLiteral/NumericLiteral node that the compiler freezes. */}
      <div onClick={-1} />
      <div onClick={NaN} />
      {/* Two distinct keys on a props object — the component boundary has no
          case folding to merge them. */}
      <Panel onClick={() => {}} onclick={() => {}} />
      <span class="spacer" />
      <span>{name()}</span>
    </div>
  );
}
