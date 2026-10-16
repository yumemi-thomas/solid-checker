/** One entry of a mount, at an absolute normalized POSIX path under its root. */
export type OwnedMountEntry =
  | { kind: "file"; path: string; bytes: Uint8Array | string }
  | { kind: "directory"; path: string }
  | { kind: "symlink"; path: string; target: string }

/**
 * A frozen copy of a project tree. Nothing above `root` exists in it; every
 * entry's parent is `root` or a declared directory; link targets resolve
 * lexically inside `root`. Modes and permissions are not represented.
 */
export interface OwnedMount {
  root: string
  entries: OwnedMountEntry[]
}

/** The native ordinary check's options, as the owned check accepts them. */
export interface OwnedCheckRequest {
  /**
   * Absolute; canonicalized as native `--project` is, against the mount or,
   * in directory mode, the host (README.md, "Project spelling").
   */
  projectId: string
  dialect?: string
  generation?: number
  runtime?: {
    target?: string
    build?: string
    rendering?: string
    programBoundary?: string
    conditions?: string[]
    frameworkTransforms?: string[]
  }
  presets?: string[]
  enableRules?: string[]
  dependencySources?: string[]
  windowRealm?: boolean
}

/** This check's producer traffic, copied after it finished. */
export interface OwnedTransportCounts {
  opened: number[]
  closed: number[]
  exchanges: number
  /** Every open, handshake, exchange and close, in order, with its status. */
  calls: Array<{ op: "open" | "handshake" | "exchange" | "close"; status: number }>
  pendingResults: number[]
}

export type OwnedCheckResult =
  | {
      ok: true
      /** The snapshot the native check prints with `--format json`. */
      snapshot: unknown
      /** The lines the native check writes to standard error. */
      notes: string[]
      /** The reactor's answer, as text. */
      json: string
      transport: OwnedTransportCounts
    }
  | { ok: false; error: string; json: string; transport: OwnedTransportCounts }

export interface OwnedChecker {
  /**
   * Checks one request. The request's JSON is fixed when this is called;
   * checks run one at a time, in call order. A refusal resolves with
   * `ok: false` and leaves the checker usable; a worker failure, trap or
   * leaked producer program rejects this and every queued check and closes
   * the checker for good.
   */
  check(request: OwnedCheckRequest): Promise<OwnedCheckResult>
  /** Rejects every pending check and stops the worker. Idempotent. */
  close(): Promise<void>
}

export declare class OwnedCheckerError extends Error {}
export declare class OwnedMountError extends Error {}

/** Starts one owned checker over `mount` (Node and Bun only). */
export declare function createOwnedChecker(options: { mount: OwnedMount }): Promise<OwnedChecker>
/**
 * Starts one owned checker over a host directory read on demand (Node and
 * Bun, POSIX hosts). `directory` is resolved to its real path once. Both
 * reactors read the host's namespace as a native check does -- the directory,
 * what lies above it, and wherever a link leads -- through one read-only store
 * that records every read. The caller keeps the path from `/` to the
 * directory, and everything a check reaches, unchanged while the checker
 * lives: a change detected after a check rejects that check and every later
 * one, and is never answered.
 */
export declare function createOwnedChecker(options: { directory: string }): Promise<OwnedChecker>

/**
 * Snapshots a directory from disk as a mount, by its real path, recording
 * links without following them. Throws on an unreadable entry, an
 * unsupported kind, or a link leaving the root. POSIX hosts only.
 */
export declare function mountDirectory(root: string): OwnedMount
