// A namespace import that escapes: the object is stored, so which of its
// exports is reached later is not something the facts can bound. The only
// member read here names a declared export, and the gap still stays open.
import * as S from "solid-js";

export const first = S.untrack(() => 1);
export const modules: object[] = [S];
