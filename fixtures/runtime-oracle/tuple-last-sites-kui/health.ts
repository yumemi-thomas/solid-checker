/**
 * Whether KUI can talk to the gateway at all, and when it will next try.
 *
 * ## Why three failures and not one
 *
 * A single failed request is not an outage. A laptop's wifi hiccups, a proxy drops one connection, a
 * phone changes cell — and if one failure took the whole application away from the user, the
 * full-screen state would flash on screen several times a day for people whose network is merely
 * ordinary. Three consecutive failures with no success in between is a much better signal, and it
 * costs at most a few seconds of delay in the case that really is an outage.
 *
 * ## Which failures count
 *
 * A success from *either* scope counts as contact: if a feature's request came back, the gateway is
 * reachable, whatever the shell's own last attempt did. A failure only counts when it is the shell's
 * own **and** it is a transport failure — a 403 or a 404 is the gateway answering, and answering is
 * the opposite of being unreachable.
 *
 * ## Why the clock and the timer are parameters
 *
 * Everything here is about time — a countdown, a doubling backoff, a cap — and a suite that waits
 * real seconds for a thirty-second cap is a suite nobody runs.
 */
import { createSignal, type Accessor } from "solid-js";

export type Connectivity =
  | { readonly kind: "connected"; readonly lastContact: Date }
  | {
      readonly kind: "lost";
      readonly since: Date;
      readonly lastContact: Date;
      readonly nextRetryInSeconds: number;
    };

/** Whose request this was. Only the shell's own failures are evidence about the gateway. */
export type CallScope = "shell" | "feature";

/** How many consecutive shell-call failures it takes. See the module comment for why it is not one. */
export const FailuresBeforeGivingUp = 3;

export const FirstBackoffMs = 2_000;

/**
 * The longest KUI ever waits between attempts.
 *
 * Low, because the cost of an extra request against a gateway that is down is negligible and the
 * cost of making a user wait while it is already back is a reload.
 */
export const MaxBackoffMs = 30_000;

/** 2 s, 4 s, 8 s, 16 s, then 30 s for ever. */
export function backoffAfter(currentMs: number): number {
  return currentMs * 2 > MaxBackoffMs ? MaxBackoffMs : currentMs * 2;
}

export type HealthOptions = {
  readonly now: () => Date;
  readonly schedule: (delayMs: number, run: () => void) => void;
  /**
   * What an attempt actually is.
   *
   * The shell passes a function that re-runs its own start-up calls; a test passes a counter.
   * Nothing here knows how to make a request.
   */
  readonly onRetry: () => void;
};

export type Health = {
  readonly connectivity: Accessor<Connectivity>;
  /** Files the outcome of one call. */
  readonly report: (scope: CallScope, outcome: "ok" | "transport-failure" | "answered") => void;
  readonly retryNow: () => void;
};

export function createHealth(options: HealthOptions): Health {
  const [connectivity, setConnectivity] = createSignal<Connectivity>({
    kind: "connected",
    lastContact: options.now(),
  });

  let consecutiveFailures = 0;
  let currentBackoffMs = FirstBackoffMs;
  /** Whether a countdown is already running, so that two failures do not start two of them. */
  let countingDown = false;
  /** Read without the signal, so the countdown does not depend on a flush having happened. */
  let current: Connectivity = connectivity();

  const publish = (next: Connectivity): void => {
    current = next;
    setConnectivity(next);
  };

  const succeed = (): void => {
    consecutiveFailures = 0;
    currentBackoffMs = FirstBackoffMs;
    countingDown = false;
    publish({ kind: "connected", lastContact: options.now() });
  };

  const tick = (): void => {
    options.schedule(1_000, () => {
      if (!countingDown) return;
      if (current.kind === "connected") {
        countingDown = false;
        return;
      }
      if (current.nextRetryInSeconds <= 1) {
        currentBackoffMs = backoffAfter(currentBackoffMs);
        publish({ ...current, nextRetryInSeconds: Math.round(currentBackoffMs / 1000) });
        options.onRetry();
      } else {
        publish({ ...current, nextRetryInSeconds: current.nextRetryInSeconds - 1 });
      }
      tick();
    });
  };

  const lose = (): void => {
    if (current.kind === "connected") {
      publish({
        kind: "lost",
        since: options.now(),
        lastContact: current.lastContact,
        nextRetryInSeconds: Math.round(currentBackoffMs / 1000),
      });
    }
    // Already lost: the original `since` is kept, because the question the user is asking is "how
    // long has this been broken?" and restamping it on every failed retry answers a different and
    // much less useful one.

    if (!countingDown) {
      countingDown = true;
      tick();
    }
  };

  return {
    connectivity,

    report: (scope, outcome) => {
      if (outcome === "ok") {
        succeed();
        return;
      }
      // "answered" is a failure the *gateway* produced, which is evidence it is reachable.
      if (outcome === "transport-failure" && scope === "shell") {
        consecutiveFailures += 1;
        if (consecutiveFailures >= FailuresBeforeGivingUp) lose();
      }
    },

    /**
     * The user pressed "Try again".
     *
     * Resetting the backoff is deliberate: a user pressing the button is evidence that they believe
     * something has changed — they have just reconnected to the wifi — and making them wait out a
     * thirty-second timer they did not choose is the sort of thing that gets an application reloaded.
     */
    retryNow: () => {
      currentBackoffMs = FirstBackoffMs;
      options.onRetry();
      if (current.kind === "lost") {
        publish({ ...current, nextRetryInSeconds: Math.round(currentBackoffMs / 1000) });
      }
    },
  };
}
