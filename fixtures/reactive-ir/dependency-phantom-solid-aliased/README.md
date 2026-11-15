# dependency-phantom-solid-aliased

**Claim (ADR 0981).** A Solid package is decided by the module an import resolves to, not by how the specifier is spelled: `dep-router` imports `my-solid`, an installed directory whose manifest is `solid-js` (an npm alias, `"my-solid": "npm:solid-js"`).

The specifier `my-solid` names no Solid package; the directory it resolves to is `solid-js`, so the import is a Solid import and `dep-router` is selected (the `package-finding-at-entry` rows are the package's own findings, which a package that is not selected has none of; main reported only SC2002). No violation: the alias has no declarations, so the compiler resolves the import to nothing and the package's reads stay uncertifiable (SC9012). The unit test `a_solid_import_is_decided_by_the_module_it_resolves_to` holds the selection itself, and the same package with the specifier spelled `solid-js` is `dependency-phantom-solid`.
