# leaf-focus-page-uppercase-solidus (ADR 1151 round 2, N1; the grammar is ADR 1154's)

**Claim.** upper case and a solidus together, `<BODY/ONFOCUSIN="hook()">`. The `focus()` in `onSettled` keeps ADR 0329's obligation unless the project's own HTML holds nothing but the program's module entries (`product_realm::page_may_run_code`: host discovery's grammar reads the page, every `<script>` is an entry, every entry is a file of the program, and no other page of the project can run code). Expected: the SC9012 at the call is **kept**.

On the build before the grammar fix (853e30b5c) the proof fired here (no SC9012 at the call): a false clean. A top-level fixture, not a variant: a project's HTML is read from its whole directory tree. The program itself (`App.tsx`) registers nothing.
