# Package integrity: package-write

ADR 0300: a selected package's sole provider and stable accessor-reading getter prove SC1001 reads; this is `dependency-source-context-getter` without its wildcard write, and the `package-write` variant withholds the getter violation (the unseen-provider variant excludes the unseen provider by premise). Solid signatures are the published rc.13 ones.
