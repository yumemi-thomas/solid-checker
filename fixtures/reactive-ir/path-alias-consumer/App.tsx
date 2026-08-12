// Precision regression for reactive-source-uncaptured (SC9011). The
// "@/helpers" specifier is spelled bare, exactly like a package import, but
// tsconfig "paths" resolves it into the project's own sources -- so passing a
// reactive accessor to it must NOT report SC9011. This guards the path-alias
// exclusion in shared_reactivity.rs's reactive_source_uncaptured (the
// `symbol_is_project_code` filter on the package-imported set): before it, a
// bare specifier alone made `consume` look like an undescribed package export
// and this file reported a false positive at `count`.
import { consume } from "@/helpers";
import { createSignal } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  return <div>{consume(count)}</div>;
}
