# solid-specifier-mapped

**Claim (ADR 1060).** The tsconfig maps `solid-js` (`paths`) to `solid-shim.ts`, a project file with Solid's names and none of its
behaviour. The compiler attests the import resolved outside every `node_modules` tree (`ImportResolution::NonRelative`), so the
specifier is not the package: no import names a primitive, and each runtime use is SC9012 `dialect-specifier-mapped`
(uncertifiable). Before ADR 1060 the imports were named after the specifier: SC4001, SC7002 ×3 and SC7001 were violations here.

Twins: `solid-specifier-mapped-package` (`paths` lands on the package's own directory: named, violations) and
`solid-specifier-mapped-other` (`paths` lands on another package: SC9012). Chrome does not apply: the oracle's Vite does not read
tsconfig `paths`, so it would run the package, not what the checker reads.
