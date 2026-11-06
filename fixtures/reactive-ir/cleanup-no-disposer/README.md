# cleanup-no-disposer

ADR 0695: with no disposer in the project a cleanup read stays certified, and so does a read in `untrack(fn)` or `onSettled(fn)`, whose window the primitive closes. `walk-bounds` holds the same cleanup with a disposer in the project, where it is uncertifiable.
