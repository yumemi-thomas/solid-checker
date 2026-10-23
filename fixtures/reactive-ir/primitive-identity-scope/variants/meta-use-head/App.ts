// `@solidjs/meta@1.0.0-next.2` is analysed from its published source. Its
// `useHead` is `@solidjs/web`'s, imported by `dist/index.js:2`: the primitive
// through that import and through the export it resolves to in
// `@solidjs/web`'s declarations (ADR 0365). `@solidjs/meta` declares no
// primitive of its own.
import { createComponent, createSignal } from "solid-js";
import { useHead } from "@solidjs/web";
import { Head, Title } from "@solidjs/meta";

export function Page() {
  return createComponent(Head, {
    get children() {
      return createComponent(Title, { children: "Home" });
    }
  });
}

// Silent: `useHead`'s tag function is tracked (`Primitive::UseHead`), so the
// signal read in it is a tracked read.
export function Titled() {
  const [title] = createSignal("Home");
  useHead(() => ({ tag: "title", props: { children: title() } }));
  return null;
}

// SC2001 violation: `@solidjs/web@2.0.0-rc.17`'s `useHead` runs the tag
// function as the compute of `effect(...)` (`dist/web.dev.js:1546-1554`), an
// owned tracked computation, so the write is a write in an owned scope. It
// is a violation only because `useHead` here is exactly `@solidjs/web`'s.
export function Writes() {
  const [title, setTitle] = createSignal("Home");
  useHead(() => {
    setTitle("Away");
    return { tag: "title", props: { children: title() } };
  });
  return null;
}
