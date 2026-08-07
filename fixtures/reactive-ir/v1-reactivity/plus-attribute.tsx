// A `+` adjacent to a quote inside attribute text. The static-string folder
// splits candidate concatenations on `+`, and a part that is a lone quote
// character used to slice out of bounds and crash the whole analysis. No
// findings are expected; this file is a crash canary.
export function PlusAttribute() {
  return (
    <div>
      <a title="+ Add" href={"a" + "b"}>
        {"+"}
      </a>
      <button aria-label={'+1'}>+</button>
    </div>
  );
}
