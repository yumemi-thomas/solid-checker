# leaf-focus-page-public-static (ADR 1151 round 2, N1; the grammar is ADR 1154's)

**Claim.** a second page of markup alone (`public/frame.html`) runs no code: the page premise holds and the `focus()` is no obligation (the control of the `public-*` pages). The `focus()` in `onSettled` keeps ADR 0329's obligation unless the project's own HTML holds nothing but the program's module entries (`product_realm::page_may_run_code`: host discovery's grammar reads the page, every `<script>` is an entry, every entry is a file of the program, and no other page of the project can run code). Expected: the SC9012 at the call is **proved**.

On the build before the grammar fix (853e30b5c) the answer is the same. A top-level fixture, not a variant: a project's HTML is read from its whole directory tree. The program itself (`App.tsx`) registers nothing.
