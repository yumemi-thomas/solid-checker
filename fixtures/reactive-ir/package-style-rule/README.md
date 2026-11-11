# package-style-rule

**Claim (ADR 0949, addendum 4).** A style rule's finding in a package analyzed as source is not handed to the project. SC8003 (`jsx-no-duplicate-props`: here, content competition, `innerHTML` beside children) says something about how the package's own JSX is written, nothing about what the project's code does at run time, so no project site has anything to answer for. Every runtime-code rule's finding in a package keeps the hand-off (an uncertifiable row of the same rule at the project site that enters the package code: `package-write-entries`, `package-read-entries`).

`App.tsx` (package `node_modules/dep-style`, a synthetic package of this fixture, source `index.jsx`):

- Violation, SC8003: `Own`, the project's own `<div innerHTML={props.html}>fallback</div>`.
- No row: `Packaged` (`<Competing />`: `Competing` holds the same SC8003 in the package) and `Quiet` (`<Fine />`, which holds nothing). The package's SC8003 is dropped under the named, typed premise `package-style-not-project` (`DropPass::PackageStyleNotProject`, `PremiseKind::PackageStyleNotProject`, written to the proof log beside the finding's package site); before this ruling it stood again as an uncertifiable SC8003 row at the element (`package-finding-at-entry:SC8003`: nine such rows in the 68 fixtures ADR 0949's addendum 1 moved).

The style rules are the three SC80xx catalog entries (`jsx-no-duplicate-props`, `prefer-for`, `prefer-show`), the same test `diagnostics.rs` applies where a style rule is exempt from a demotion (`id.starts_with("SC80")`).
