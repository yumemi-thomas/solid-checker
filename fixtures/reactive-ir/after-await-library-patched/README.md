# after-await-library-patched

The calls of `after-await-library`'s `Outside` in a program that writes `String.prototype.concat` (ADR 0976): one written built-in anywhere takes the proof from every standard call, construct and setter, so each call keeps SC1002's uncertifiable row `read-after-await-unaccounted`. The twin of the clean fixture; see its README for the clauses of the fact.
