// app-game components ui/button.tsx: the whole props object is spread onto
// the element.
import type { ComponentProps } from "@solidjs/web";

function Button(props: ComponentProps<"button">) {
  return <button type="button" {...props} />;
}

export function Toolbar() {
  return <Button aria-label="Close">×</Button>;
}
