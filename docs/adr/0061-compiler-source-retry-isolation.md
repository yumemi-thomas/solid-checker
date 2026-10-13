# Compiler source acquisition across certification retries

Independent case recovery repeatedly calls native certification within one
orchestration transaction. Each call creates a new compiler-source collector,
whose source counter starts at zero. The collectors previously shared
`scratch/root-sources`, and each used exclusive directory creation for names
such as `root-source-0`. Later calls encountered existing directories; those
errors flowed into the unavailable-source disposition and removed otherwise
authenticated dependency packages from the private Type Facts program.

Each acquisition now gets a new temporary directory beneath the transaction's
scratch root. There is no cross-attempt directory reuse, overwrite, or cleanup
of an earlier attempt. Registry caches still validate their existing exact
package/version/integrity/origin identities; native archive replay, installed
package roots, lock locators, importer contexts and source-file digests remain
unchanged. A genuine acquisition or identity failure still withholds the
package. No protocol, receipt format, behavior rule or trust policy changes.

The motivating Kobalte experiment used its retained lock-selected package set
and the existing registry cache with network disabled. The first acquisition
returned 18 compiler-source packages; repeating it in the same scratch root
lost three, including `solid-js`. Independently, both TypeScript 5.9.3 and the
repository's Type Facts producer classified the real installed
`ColorModeContext` declaration as non-callable and non-constructable. This
established a retry/materialization defect before considering any new proof
rule for the generic context type.

The focused regression uses the same scratch root across repeated acquisitions
and checks exact package identities, separate archive paths, and preservation
of earlier bytes. A later genuine acquisition failure still excludes that
package. It failed before the patch and the complete contract-workflow suite
passes afterward (83 tests). Corpus gains are measured separately; fixing
acquisition is not itself a new certification or a denominator correction.
