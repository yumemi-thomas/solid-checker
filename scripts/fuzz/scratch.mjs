// A grid's ephemeral directories in a scratch directory (`focus.mjs --scratch`, the VM's tmpfs): a grid writes a directory per program
// (`cases/`, `chrome-cases/`) and its checkers a role log of up to gigabytes, all deleted once the grid is folded (`--prune-cases`). On
// the VM's disk every one of those writes is a write to a disk image in the host's home directory, which the host's fseventsd follows
// (scripts/fuzz/README.md, The Linux VM). Each lives in the scratch directory and is reached through a link at its usual path, so every
// path a grid reads or records (a finding's `repro`, roles.json's `log`) is the one it had on disk; only the bytes move.
import { existsSync, lstatSync, mkdirSync, realpathSync, rmSync, symlinkSync } from "node:fs";
import { dirname, join } from "node:path";

/** A grid directory's ephemeral subdirectories (what `--prune-cases` deletes): the program cases and Chrome's copies of the sampled ones. */
export const SCRATCH_DIRS = ["cases", "chrome-cases"];

const isLink = (path) => {
  try { return lstatSync(path).isSymbolicLink(); } catch { return false; }
};

/**
 * Links `<dir>/<name>` for each of SCRATCH_DIRS into `scratch`, replacing what was there (a grid's `--scratch`, matrix.mjs and
 * legal.mjs, right after they create their fresh `--out`).
 */
export function linkScratch(dir, scratch) {
  mkdirSync(dir, { recursive: true });
  for (const name of SCRATCH_DIRS) {
    const target = join(scratch, name);
    removeLinked(join(dir, name));
    rmSync(target, { recursive: true, force: true });
    mkdirSync(target, { recursive: true });
    symlinkSync(target, join(dir, name));
  }
}

/**
 * Links a grid's role log `<dir>.roles.jsonl` (next to the grid directory, which focus.mjs leaves for the grid to create) into
 * `scratch`. Returns the log's path there: the checkers write it there and the fold reads it through the link.
 */
export function linkRoleLog(dir, scratch) {
  mkdirSync(scratch, { recursive: true });
  mkdirSync(dirname(dir), { recursive: true });
  const log = join(scratch, "roles.jsonl");
  removeLinked(`${dir}.roles.jsonl`);
  rmSync(log, { force: true });
  symlinkSync(log, `${dir}.roles.jsonl`);
  return log;
}

/** Deletes `path` and, when it is a link, what it points at (a scratch directory or log). */
export function removeLinked(path) {
  if (isLink(path)) {
    let target = null;
    try { target = realpathSync(path); } catch { /* a dangling link: the scratch is gone (a reboot emptied the tmpfs) */ }
    if (target) rmSync(target, { recursive: true, force: true });
  }
  rmSync(path, { recursive: true, force: true });
}

/** `path` as an empty directory; a link (linkScratch) stays a link and its target is emptied. */
export function freshDir(path) {
  if (isLink(path) && existsSync(path)) {
    const target = realpathSync(path);
    rmSync(target, { recursive: true, force: true });
    mkdirSync(target, { recursive: true });
    return;
  }
  rmSync(path, { recursive: true, force: true });
  mkdirSync(path, { recursive: true });
}
