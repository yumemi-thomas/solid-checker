# release-triple-solid-free-rc3

A project with no Solid code beside an unaudited `2.0.0-rc.3` triple: nothing names a Solid package, there is no JSX and no reached package depends on one, so no release-dependent question is asked (`release_scope.rs`, `solid_use`). Expected: no finding, `certified`, no `SC9014` (`the_release_notice_is_due_only_for_a_project_that_uses_solid` in `dialects_process.rs` asserts the notice count); controls keeping the notice: `release-triple-solid-import-rc3`, `-jsx-only-rc3`, `-solid-dependent-rc3`.
