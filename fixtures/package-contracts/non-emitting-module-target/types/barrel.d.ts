// `@solidjs/universal@2.0.0-rc.3`'s `types/index.d.ts` and `@solidjs/h`'s, in
// one member: a declaration file emits no module at all, so it emits no
// re-export either — not even one naming `default`. The identical bytes in a
// runtime member are a working barrel, and the control fixture pins that they
// refuse there.
export * from "./renderer.js";
export { type RendererOptions } from "./renderer.js";
