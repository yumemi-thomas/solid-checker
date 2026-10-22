# setter-in-render-root

A setter in `render`'s `code` is SC2001 on rc.13 and rc.17 `@solidjs/web` (rc.17 `dist/web.dev.js:551-558`; ADR 0353); in `hydrate`'s it is uncertifiable on rc.17, whose `hydrate` may run `code` after it returns (`:1631-1645`). Negatives: a `createEffect` apply, a shadowed `render`. A listener is uncertifiable. Twin `setter-in-render-root-rc9`.
