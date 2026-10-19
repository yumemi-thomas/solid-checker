# A consumer of a described callable's callback items

ADR 0152: an accepted contract whose closed `returns` hands back a described callable running the export's own arguments, with closed `callbacks` at `result-access`, places a callback at such an argument wherever the returned function is called: in a body `SC1001`, in JSX or unused clean, with open `callbacks` `SC9005` (never a proven read). `pipe` is declared as `@solid-primitives/utils@7.0.0-next.4` declares it (the same `App.tsx` with the published package beside `solid-js`/`@solidjs/web` `2.0.0-rc.9` is `tsc` clean).
