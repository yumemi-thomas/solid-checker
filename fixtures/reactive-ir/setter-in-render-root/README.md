# setter-in-render-root

A setter in `render`'s or `hydrate`'s `code`, inline or by name, is SC2001 on rc.13 and rc.17 `@solidjs/web` (ADR 0353): rc.17 `hydrate` may run `code` after it returns (`dist/web.dev.js:1631-1645`), always under `render`'s root (`:551-558`). Negatives: a `createEffect` apply, a click handler, a shadowed `render`. A listener is uncertifiable. Twin `setter-in-render-root-rc9`.
