import * as S from "solid-js";

// The namespace object is handed on as an array element: any member may be
// read through it, so the program may name every primitive and the flush
// keeps its rows.
export const modules = [S];

export function Member() {
  return (
    <button onClick={() => {
      S.flush();
    }}>go</button>
  );
}
