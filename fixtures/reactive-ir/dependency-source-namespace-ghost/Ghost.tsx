import * as Widget from "dep-ns/widget";

// UNCERTIFIABLE (SC9005): `ghost` is declared but the selected runtime file
// does not export it, so it binds to no source. Its whole member span
// `Widget.ghost` names the namespace (the leading token), which `readLater`'s
// redirect bound, so the member counted as bound (before: certified).
export function Ghost() {
  Widget.readLater(() => 0);
  const now = Widget.ghost(() => 1);
  return <span>{now}</span>;
}
