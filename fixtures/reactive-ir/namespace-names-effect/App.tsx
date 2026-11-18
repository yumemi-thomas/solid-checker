import * as S from "solid-js";

// The misuse twin: the namespace names `createEffect`, whose apply a flush
// drains, and the flush written in the apply is SC2008's violation.
export function InApply() {
  S.createEffect(
    () => 1,
    () => {
      S.flush();
    },
  );
  return <div />;
}
