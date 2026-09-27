// A namespace import whose every use is a member read: `S.untrack` reaches an
// export rc.9 declares, `S.createLoadingBoundary` one it does not.
import * as S from "solid-js";

export const first = S.untrack(() => 1);
export const loading = S.createLoadingBoundary;
