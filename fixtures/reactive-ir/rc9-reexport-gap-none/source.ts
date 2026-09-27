// Reaches solid-js, and none of the five names rc.9's typings re-export
// without declaring: a named import of another export, and a namespace import
// read only as a member that is not one of them.
import { untrack } from "solid-js";
import * as S from "solid-js";

export const first = untrack(() => 1);
export const second = S.untrack(() => 2);
