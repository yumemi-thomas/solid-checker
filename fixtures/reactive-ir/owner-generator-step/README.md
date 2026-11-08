# A generator's later step is no owned run (ADR 0866)

A generator's body runs where its iterator is stepped. An owner operation after a
`yield` runs on a later `.next()`, which the owner walk does not follow: here a
timer's, where rc.21 raises NO_OWNER_CLEANUP. It is uncertifiable, whatever owner
the generator's call had. An operation before the first `yield`, stepped in the
body, stays owned. A helper called past a `yield` takes that run from the call's edge (reviewer R60), also when the body calls it too.
