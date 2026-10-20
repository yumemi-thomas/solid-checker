# Two router copies in one program (ADR 0324)

`packages/a` resolves `@solidjs/router` to the audited `2.0.0-next.26`; `packages/b` to a copy whose manifest says `2.0.0-next.18` (unaudited; files are `next.26`'s with only the version changed). The premise reads each import's own manifest, so one unaudited copy leaves it unapplied project-wide: both accessors keep `SC9012` and `SC9014` names the import.
