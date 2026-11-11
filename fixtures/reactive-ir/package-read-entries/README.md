# package-read-entries

**Claim (ADR 0949).** A package function that reads a signal runs that read in the project's untracked scope wherever the project enters it: by `f.call(null)`, a tagged template, `new`, a JSX tag, or a prop the package reads. Before ADR 0949 a package function whose uses were not plain calls was concluded to be entered by nothing (the references of the package's runtime symbol were empty: a project's use names the declaration's `.d.ts` symbol), so its read ended silently. Chrome (rc.21 development build): `STRICT_READ_UNTRACKED` at the project line of each positive.

`App.tsx` (package `node_modules/dep-read`, a synthetic package of this fixture):

- Obligation (SC1001, uncertifiable, `package-finding-at-entry:SC1001`): `Dotted` (`count.call(null)`), `Tagged` (``count`x` ``), `Constructed` (`new Counter()`), `Rendered` (`<Own />`), `Prop` (`<Panel value={n()} />`: the package body reads `props.value`).
- Silent: `Tracked` (`quiet()` in an effect's compute function reads under a tracking owner: legal, and the package holds no finding in `quiet`).

The build before ADR 0949 certified the whole project clean (0 rows) while Chrome fired the code at the five positives.

`foreign_reference_hand_offs` and the proof "no function enters it" (`function_nothing_enters`) now ask the references of every symbol a runtime redirect ties to the function (`SemanticLookup::symbol_references_through_redirects`), so a package function with a project use is never "entered by nothing".
