import { createMemo } from "solid-js";
import { helpers as Api } from "./barrel";
const slow = createMemo(async () => { await Promise.resolve(); return 1; });
export function App() {
  return <p>{(() => { Api.run(() => { slow(); }); return 1; })()}</p>;
}
