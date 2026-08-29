# Unselected conditional side-effect modules remain open

`index.js` imports `#platform` only for side effects. The package `imports` map
can select `platform-browser.mjs` or `platform-node.mjs`, while the generated
artifact case selects neither condition. Both files are real runtime targets
that could change the negative claims of the entry module.

The stable-v1 producer does not choose a branch or treat TypeScript's
unresolved answer as proof that no runtime module executes. The affected
artifact retains ten local unresolved claims and no proposed closure candidate.
By contrast, `asset-import` names no executable runtime target and does not
create this conditional-module hazard.
