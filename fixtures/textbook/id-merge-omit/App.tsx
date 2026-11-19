import { merge, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

function Button(props: { variant?: "primary" | "plain" } & JSX.ButtonHTMLAttributes<HTMLButtonElement>) {
  const merged = merge({ variant: "plain" as const, type: "button" as const }, props);
  const rest = omit(merged, "variant", "children");
  return (
    <button {...rest} class={merged.variant === "primary" ? "primary" : "plain"}>
      {merged.children}
    </button>
  );
}

export function Toolbar() {
  return (
    <div>
      <Button variant="primary" onClick={() => console.log("save")}>
        save
      </Button>
      <Button>cancel</Button>
    </div>
  );
}
