import { merge, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

function Badge(props: { label?: string; tone?: string } & JSX.HTMLAttributes<HTMLSpanElement>) {
  const withDefaults = merge({ label: "new", tone: "info" }, props);
  const rest = omit(withDefaults, "label", "tone");
  return (
    <span {...rest} data-tone={withDefaults.tone}>
      {withDefaults.label}
    </span>
  );
}

export function Badges() {
  return (
    <div>
      <Badge />
      <Badge label="hot" tone="warn" title="hint" />
    </div>
  );
}
