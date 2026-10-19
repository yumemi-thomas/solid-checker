# A tracked callback that may never run

ADR 0244: a contract may state a callback runs tracked under an owner the export creates, but possibly zero times (`createLazyMemo`'s calc), so an accessor read written in it is not an untracked read (the caller's own body read stays a violation; the optional write is not upgraded); `satisfies`-wrapped, untracked/mixed/open slots, nested/async/generator/named frames, shadows and returned-callback escapes are uncertifiable. Authorized via `.solid-checker/authorize-contract.json` as in `package-own-tracked-read-consumer`.
