# Inferred host: static loadability and reachability

A browser host is inferred only from exact plain Vite commands in the app's own manifest (ADR 0270); launcher/shell/env/filter variants, enclosing-manifest Vite scripts, symlinks and active tsconfig-paths resolvers refuse. A module not proved loadable keeps the baseline; a literal throw proves its tail dead, an ordinary earlier call does not (premise A). `isServer` is byte-faithful to `@solidjs/web@2.0.0-rc.13`'s published boolean signature; no stub is loosened.
