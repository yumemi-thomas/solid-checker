# suppression-comments

Claim (ADR 0429): a suppression comment removes the violation at its site, under every spelling: `eslint-disable-next-line`, `eslint-disable-line`, a block `eslint-disable` ... `eslint-enable`, with `solid-checker/<rule>`, `solid/reactivity`, `solid-checker/reactivity`, a name list, and the `solid-checker-*` family with a bare name, an SC code and a prefixed name; a bare comment names every rule; a JSX `{/* */}` comment counts. Each component holds one untracked read (SC1001). Only the four reported sites remain: `Plain` (no comment), the site after `BlockRange`'s `eslint-enable`, the site after `CheckerBlockRange`'s `solid-checker-enable`, and the unsuppressed button in `JsxComment` (SC1007).

Not covered here: what a comment does not suppress (`suppression-limits`), and unknown names (`suppression-unknown-name`).
