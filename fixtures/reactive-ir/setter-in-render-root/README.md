# setter-in-render-root

A setter in `render`'s or `hydrate`'s `code` is SC2001 on rc.13 `@solidjs/web` (`dist/web.dev.js:488-495`; ADR 0353). Negatives: a `createEffect` apply, a shadowed `render`. A listener is uncertifiable. Twin `setter-in-render-root-rc9`.
