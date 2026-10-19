# A consumer of a plain return

Written (ADR 0113) to pin that a contract closing `returns` over one `plain` return leaves nothing open at the import, while the `StatusOpen`/`isReadyOpen` control raises `SC9005`. Package contracts are removed (ADR 0312). Today the snapshot holds only the SC9014 notice for the rc.0 stub: the stub package declares no `solid-js` peer dependency, no SC9005 is raised for either export, and the control no longer differs from the closed case, so the fixture pins nothing about plain returns. The `solid-reactivity.json` beside the stub is inert.
