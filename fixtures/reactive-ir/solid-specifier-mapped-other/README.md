# solid-specifier-mapped-other

**Claim (ADR 1060).** `paths` maps `solid-js` into `node_modules/other-signals`, whose manifest names `other-signals`: the
declarations match Solid's, but the package is not `solid-js`, so nothing is named and each use is SC9012
`dialect-specifier-mapped`. Before ADR 1060 every claim here was a violation.
