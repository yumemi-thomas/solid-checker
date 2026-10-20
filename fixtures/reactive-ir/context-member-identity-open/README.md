# Context member identity, open program

Control for `context-member-identity`. Without a manifest the program is open: `useContext` is a channel any code
beneath a provider can use, so an unseen importer can provide `TabsContext`/`AccountContext` itself or reach the
shared value through `useTabs`/`useAccount` (or a returned closure) and replace a member where ADR 0273's name index
cannot see the write. No context's provider census is closed, whether published by an export list (`tabs.ts`) or
`export const` (`account.ts`), so neither read is attributed to its signal.
