import { Loading, createEffect, createMemo } from "solid-js";
import type { Accessor } from "solid-js";

function LoadingChild(props: { data: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>{props.data()}</p>;
}

// Uncertifiable (in `LoadingChild`): the tree reads the memo, so the boundary
// shows its fallback and holds the user effect until the reveal, when the
// memo has settled (rc.17 warns STRICT_READ_UNTRACKED instead).
export function LoadingHolder() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <Loading fallback={0}>
        <LoadingChild data={data} />
      </Loading>
    </div>
  );
}
