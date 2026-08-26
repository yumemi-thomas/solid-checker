# Entrypoint condition isolation

The root entrypoint selects a server implementation whose `sameName` export
returns its argument and a default web implementation which returns
`undefined`. The `./jsx-runtime` entrypoint always selects only the web file.

Sharing one runtime target does not make two public entrypoints aliases. A
relational return proved for the root's server condition must stay on that
condition and must not contaminate the unconditional JSX-runtime summary.
