# Shallow callback-result Get

ADR 0263: `readResult` states a closed result-use census (a root Get and own-value enumeration at call), so plain data-property results lose result-dispatch uncertifiability under ADR 0190's unpatched-prototype premise; `ProducerViolation` keeps SC2001; accessors, methods, spreads, computed keys, `__proto__`, arrays, proxies, dynamic returns, async producers, open censuses and escaped receivers stay SC9012. Stubs/manifest byte-identical to `package-scalar-result-consumer`.
