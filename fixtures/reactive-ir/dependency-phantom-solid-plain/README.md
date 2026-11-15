# dependency-phantom-solid-plain

**Claim (ADR 0981).** The selection does not over-reach: a package whose closure imports no Solid package (`dep-plain`) stays what it was: not selected. (A package whose only `solid-js` import is a type is held by the unit test `a_package_importing_no_solid_package_stays_unselected`.)

The package is not read as source, so the value it returns is read under the rule of an unselected package: the read `parsed.value` in the component body is an uncertifiable SC1001 row (`package-value-read`), never a violation (nothing proves the value is reactive, and nothing proves it is not; a package that returns what the program hands it is the case the rule exists for, see `dependency-phantom-solid-wrap`).
