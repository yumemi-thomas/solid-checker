# Source callable identity in leaf scopes

The leaf checker uses the same exact callable transport as source read analysis: returned arrows and receiver-inert shorthand methods keep their implementation identity across files and their bodies are still walked for forbidden operations (clean bodies clean, forbidden/nested primitives proven, conditional/default execution uncertain); `SelectedInApply` keeps ADR 0320's unmodeled-use obligation. Receiver-dependent, overwritten and escaped methods are in `leaf-source-call-targets-negative`. Callback return shapes match published rc.13 declarations.
