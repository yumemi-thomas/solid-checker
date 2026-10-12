# Context member identity, open program

Control for `context-member-identity`. Without a manifest the program is
open. useContext is a channel any code beneath a provider can use: an unseen
importer can provide `TabsContext`/`AccountContext` itself, or reach the
shared value through `useTabs`/`useAccount` (or a returned closure) and replace
a member where ADR 0273's name index cannot see the write. No context's
provider census is closed, whether the context is published by an export list
(`tabs.ts`) or by `export const` (`account.ts`), so neither read below is
attributed to its signal.
