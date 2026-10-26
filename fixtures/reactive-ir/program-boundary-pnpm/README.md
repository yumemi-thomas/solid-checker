# program-boundary-pnpm

**Claim (ADR 0444).** A `pnpm-workspace.yaml` lists the members as `workspaces` does, and a path protocol counts as a dependency: `variants/shell` depends on `variants/kernel` by `link:../kernel`, so `variants/kernel` is open and its `bump` write stays an `SC2001` obligation; `variants/shell`, which nothing depends on, is closed and certified.
