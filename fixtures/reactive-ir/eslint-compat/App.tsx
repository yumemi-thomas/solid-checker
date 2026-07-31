import { createSignal } from "solid-js";

const [name] = createSignal("");

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
      {/* SC8004: eval with a different spelling */}
      <a href="javascript:void(0)">dismiss</a>
      {/* SC8004 again: whitespace and control characters do not disarm it */}
      <a href="java&#9;script:alert(1)">tricky</a>
      {/* SC8008: parses as HTML and discards children */}
      <div innerHTML={"<b>x</b>"} />
      {/* SC8011: three React names Solid does not forward */}
      <label className="field" htmlFor="name" key="k">Name</label>
      {/* SC8012: not a Solid namespace, so not a directive */}
      <input model:value={name()} />
      {/* SC8016: childless, and whitespace is not a child */}
      <span class="spacer"></span>
      <span>   </span>
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
      {/* Every namespace Solid's compiler recognises. */}
      <input
        prop:value={name()}
        attr:data-x="1"
        bool:disabled={true}
        on:click={() => {}}
        oncapture:click={() => {}}
        use:autofocus={true}
        style:color="red"
      />
      <span class="spacer" />
      <span>{name()}</span>
    </div>
  );
}
