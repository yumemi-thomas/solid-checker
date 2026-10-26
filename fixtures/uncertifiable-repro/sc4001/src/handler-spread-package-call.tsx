// probus-hk src/routes/Rail.tsx:39: the props a package function returns
// (`useLinkProps` of @tanstack/solid-router there) are spread onto an element,
// so the listeners the spread brings are the package function's to say.
import { globalRegistry } from "@solid-primitives/utils";

export function Link(props: { code: string }) {
  return (
    <a {...globalRegistry(`link-${props.code}`, () => ({ onClick: () => console.debug("open"), href: "/rail" }))} class="link">
      {props.code}
    </a>
  );
}
