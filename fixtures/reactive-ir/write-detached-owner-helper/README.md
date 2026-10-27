# write-detached-owner-helper

ADR 0457: a `runWithOwner` callback's literal handed to a function proven to call it inline (project or dependency source) takes the call's owner. Cases and rc.17 evidence: comments in `App.tsx` (the fuzzer shape) and `Package.tsx` (synthetic packages).
