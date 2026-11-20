# leaf-focus-page-extra-script (ADR 1151, round 1)

**Claim.** a second, classic `<script src>`: a script that is not the program's module entry. The `focus()` in `onSettled` keeps ADR 0329's obligation unless the project's own HTML holds nothing but the program's module entries
(`product_realm::page_may_run_code`: host discovery's grammar reads the page, every `<script>` is an entry, every entry is a file of the program). Expected: the
SC9012 at the call is **kept**.

A project's HTML is read from its whole directory tree, so these are projects of their own, not variants nested under `leaf-focus-no-listener` (whose program would
see their pages). The program itself (`App.tsx`) registers nothing.
