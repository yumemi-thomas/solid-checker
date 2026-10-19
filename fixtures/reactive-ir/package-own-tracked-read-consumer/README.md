# A package's own tracked read

ADR 0239: a contract may state a read of the export's own reactive state; where it runs untracked on the call's stack Solid warns at every use and the checker reports it as the package's own read (`SC1001`, uncertifiable), and where the contract states it `tracked` under an owner the export creates it runs in the export's own computation and nothing is reported (`Watched`/`watchStatus` clean, `Peeked`/`peekStatus` uncertifiable). Authorized via `.solid-checker/authorize-contract.json` as in `package-merged-props-consumer`.
