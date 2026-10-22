# Variant: `Function.prototype.bind` patched

Same claim as `../../README.md`, one change: `patch.ts` replaces `Function.prototype.bind` and gives each bound function `length` 1. Every rc.13 getter is `read.bind(null, node)`, so one created after the patch has length 1 and `access` returns it uncalled; the length fact is withheld for the whole project (ADR 0338), and `access(value)` stays uncertifiable. Packages resolve from the parent fixture's `node_modules` by Node's lookup. The residual premise is that no builtin is patched by means the tripwire cannot see.
