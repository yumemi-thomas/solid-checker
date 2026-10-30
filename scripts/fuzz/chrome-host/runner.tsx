// The Chrome host's in-page runner: fast/runner.tsx moved into a real browser, its error handling the oracle entry's. Same capture (@solidjs/diagnostics captureArtifact
// and an OBSERVE.diagnostics listener that stacks each event), same driving (every export rendered in its own root, two passes of
// DOM events on every element, the plan's edits and clicks, the hit ledger settled), same record. Two host differences only: the
// plan is fetched instead of read from disk, and the record resolves `window.__scRecordPromise` (index.html; chrome-host.mjs awaits it and maps its
// stack positions through Vite's source maps: in Chrome an Error.stack names positions in the served, transformed module).
import { captureArtifact } from '@solidjs/diagnostics';
import { OBSERVE, flush } from 'solid-js';
import { render } from '@solidjs/web';
import { installVirtualClock } from './virtual-clock.mjs';

const params = new URLSearchParams(location.search);
const id = params.get('id') as string;
const settings = { eventPasses: 2, pauseMs: Number(params.get('pause') ?? 60), finalMs: Number(params.get('final') ?? 150) };
let clock: ReturnType<typeof installVirtualClock> | null = null;
const pause = (ms: number) => (clock ? clock.advance(ms) : new Promise<void>(resolve => setTimeout(resolve, ms)));
const describeError = (e: any) => ({
  message: String(e?.message ?? e),
  stack: String(e?.stack ?? ''),
  cause: e?.cause ? { message: String(e.cause?.message ?? e.cause), stack: String(e.cause?.stack ?? '') } : null,
});

/** The App.tsx frames of a stack as served (transformed) positions, innermost first; chrome-host.mjs maps them. */
function servedFrames(stack: string) {
  const rows: { file: string, line: number, column: number }[] = [];
  for (const text of String(stack ?? '').split('\n')) {
    const match = new RegExp(`/cases/${id}/App\\.tsx(?:\\?[^:\\s)]*)?:(\\d+):(\\d+)`).exec(text);
    if (match) rows.push({ file: 'App.tsx', line: Number(match[1]), column: Number(match[2]) });
  }
  return rows;
}

function events() {
  return [
    ...['click', 'dblclick', 'mousedown', 'mouseup', 'mouseover', 'mouseout', 'pointerdown', 'pointerup', 'contextmenu']
      .map(type => [type, () => new MouseEvent(type, { bubbles: true, cancelable: true })] as const),
    ...['keydown', 'keyup'].map(type => [type, () => new KeyboardEvent(type, { bubbles: true, key: 'a' })] as const),
    ...['focus', 'blur', 'focusin', 'focusout'].map(type => [type, () => new FocusEvent(type, { bubbles: true })] as const),
    ...['input', 'change', 'submit', 'scroll', 'wheel'].map(type => [type, () => new Event(type, { bubbles: true, cancelable: true })] as const),
  ];
}

async function settleHits(): Promise<boolean | null> {
  const g = globalThis as any;
  if (!g.__scHits) return null;
  let last = JSON.stringify(g.__scHits), still = 0;
  for (let waited = 0; waited < 2000; waited += 50) {
    await pause(50);
    const now = JSON.stringify(g.__scHits);
    if (now === last) { still += 1; if (still >= 2) return true; } else { still = 0; last = now; }
  }
  return false;
}

// The runner settles the page after a plan step with flush(). In the oracle, Playwright's click returns and Solid flushes on its own
// schedule, so an error the flush raises (an `onSettled` cleanup in an unowned scope, say) is an uncaught page error and the page
// goes on; the same error here is rethrown from a timer, never ends the run.
function settle() {
  try { flush(); } catch (e) { setTimeout(() => { throw e; }); }
}

async function runCase() {
  const started = performance.now();
  const diagnostics: any[] = [], errors: any[] = [], consoleErrors: string[] = [], disposers: (() => void)[] = [];
  let witnesses: any[] = [], failure: any = null, phase = 'import', artifactCount: number | null = null;
  let hits: Record<string, number> | null = null, order: number[] = [], hitsDom: string | null = null, quiescent: boolean | null = null;
  // The oracle's stack depths: a page error keeps Chrome's default (10 frames), and a diagnostic's stack is captured with 40
  // (packages/cli/scripts/feedback-read-runtime.mjs `maxFrames`), so an application frame below either depth is lost the same way.
  const onError = (event: ErrorEvent) => {
    // Chrome's ErrorEvent.message reads "Uncaught Error: <message>"; the error's own message is what Happy DOM and the oracle record.
    errors.push({ phase, kind: 'window-error', message: String(event.error?.message ?? event.message), stack: String(event.error?.stack ?? '') });
    event.preventDefault();
  };
  const onRejection = (event: PromiseRejectionEvent) => {
    errors.push({ phase, kind: 'unhandledrejection', ...describeError(event.reason) });
    event.preventDefault();
  };
  // A timer callback that throws reaches the window in a browser; on the virtual clock it is reported the same way.
  if (params.get('clock') === 'virtual') clock = installVirtualClock({ onError: (e: any) => errors.push({ phase, kind: 'window-error', ...describeError(e) }) });
  const oldError = console.error, oldWarn = console.warn;
  console.error = (...args) => { consoleErrors.push(args.map(x => String(x)).join(' ')); };
  console.warn = () => {};
  window.addEventListener('error', onError);
  window.addEventListener('unhandledrejection', onRejection);
  const diagnosticStack = (code: string) => {
    const limit = Error.stackTraceLimit;
    Error.stackTraceLimit = 41;
    try { return new Error(`diagnostic ${code}`).stack; } finally { Error.stackTraceLimit = limit; }
  };
  const stop = OBSERVE!.diagnostics.subscribe((event: any) => {
    diagnostics.push({ code: event.code, message: event.message ?? null, loc: event.loc ?? null, siteKind: event.siteKind ?? null,
      phase, stack: diagnosticStack(event.code) });
  });
  try {
    const plan = await (await fetch(`/cases/${id}/sites.json`)).json();
    const captured = await captureArtifact(async () => {
      const mod = await import(/* @vite-ignore */ `/cases/${id}/App.tsx`);
      const root = document.createElement('div'); root.id = 'root'; document.body.append(root);
      phase = 'render';
      for (const [name, value] of Object.entries(mod).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
        if (typeof value !== 'function' || !(name === 'default' || /^[A-Z]/.test(name))) continue;
        const Component = value as any;
        const host = document.createElement('section'); host.dataset.export = name; root.append(host);
        // As the oracle's entry (checker.mjs oracleMainSource): a render that throws is logged and rethrown from a timer, so it
        // reaches the page as an uncaught error, which is how the oracle records it (a runtime exception at its throwing site).
        try { disposers.push(render(() => <Component />, host)); }
        catch (e) { consoleErrors.push(`render failed ${name} ${String((e as any)?.message ?? e)}`); setTimeout(() => { throw e; }); }
      }
      phase = 'exercise';
      await pause(settings.pauseMs);
      for (let pass = 0; pass < settings.eventPasses; pass++) {
        for (const element of Array.from(root.querySelectorAll('*'))) {
          for (const [type, make] of events()) {
            // As the oracle's entry: a dispatch that throws is logged, never an uncaught error (the oracle records none).
            try { element.dispatchEvent(make()); } catch (e) { consoleErrors.push(`handler threw ${type} ${String((e as any)?.message ?? e)}`); }
          }
        }
        await pause(settings.pauseMs);
      }
      await pause(settings.finalMs);
      phase = 'plan';
      for (const edit of plan.plan.edits ?? []) {
        const target = document.querySelector(edit.selector);
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.focus(); target.value = edit.text; target.dispatchEvent(new Event('input', { bubbles: true })); settle(); await pause(settings.pauseMs);
        } else errors.push({ phase, kind: 'plan', message: `no input ${edit.selector}`, stack: '' });
      }
      for (const buttonId of plan.plan.clicks) {
        const target = document.querySelector(`[data-act="${buttonId}"]`);
        if (target instanceof HTMLElement) { target.click(); settle(); await pause(settings.pauseMs); }
        else errors.push({ phase, kind: 'plan', message: `no button ${buttonId}`, stack: '' });
      }
      await pause(settings.finalMs);
      quiescent = await settleHits();
      phase = 'witness';
      const counted = (globalThis as any).__scHits;
      if (counted) { hits = { ...counted }; order = ((globalThis as any).__scOrder ?? []).slice(); }
      hitsDom = document.getElementById('sc-hits')?.textContent ?? null;
      witnesses = plan.sites.map((site: any) => {
        if (!site.witness) return { siteId: site.id, witness: null, present: false, actual: null, reached: null };
        const element = document.querySelector(site.witness.selector);
        const actual = element ? (element.textContent ?? '').trim().replace(/\s+/g, ' ') : null;
        return { siteId: site.id, witness: site.witness, present: element !== null, actual, reached: element === null ? null : actual === site.witness.text };
      });
    }, { scenario: id, attribution: false, autoFlush: false });
    artifactCount = captured.artifact?.diagnostics?.length ?? null;
  } catch (e) { failure = { phase, ...describeError(e) }; }
  finally {
    clock?.restore(); clock = null;
    stop();
    for (const dispose of disposers) try { dispose(); } catch (e) { errors.push({ phase: 'dispose', kind: 'dispose', ...describeError(e) }); }
    window.removeEventListener('error', onError);
    window.removeEventListener('unhandledrejection', onRejection);
    console.error = oldError; console.warn = oldWarn;
    (window as any).__scResolve({
      id, wallMs: Math.round(performance.now() - started), failure, witnesses, hits, order, hitsDom, quiescent,
      diagnostics: diagnostics.map(d => ({ code: d.code, message: d.message, loc: d.loc, siteKind: d.siteKind, phase: d.phase, positions: servedFrames(d.stack) })),
      errors: errors.filter(e => e.phase !== 'dispose').map(e => ({ phase: e.phase, kind: e.kind, message: e.message, cause: e.cause?.message ?? null,
        positions: [...servedFrames(e.stack), ...servedFrames(e.cause?.stack)] })),
      consoleErrors: consoleErrors.slice(0, 20), artifactCount,
    });
  }
}


void runCase();
