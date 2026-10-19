# `omit` views read through to their props (ADR 0323)

An `omit` view's first argument is its source: a read of a key the view keeps is a props read, **always uncertifiable**; a key hidden by a string literal reads nothing; predicates and non-literal keys hide nothing nameable; spreading a view (or `merge` view) is an obligation, never a violation. `Write` is an uncertifiable `SC2003`: `omit` of a proxy drops the write (`@solidjs/signals@2.0.0-rc.13` `dist/prod/store/utils.js:850`), of the compiler's plain props copies it (`dist/dev.js:4573-4600`), and which is unproven.
