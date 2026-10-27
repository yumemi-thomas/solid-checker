// The in-process runtime runner: mounts one generated case under Happy DOM with the
// Solid dev build, drives it as scripts/runtime-oracle.mjs does (two passes of DOM
// events on every element, then the case's plan steps), and writes one JSON record:
// the dev diagnostics (code, message, owner loc, and the stack lines that fall in
// App.tsx), the errors thrown, and the page text at every witness selector.
//
// Ported from the Happy DOM experiment's runner (happy-patched-runner.tsx): the capture mechanism
// (@solidjs/diagnostics captureArtifact plus an OBSERVE.diagnostics listener that
// stacks each event) and the event passes are its; its corpus-specific scenario
// steps and case lookup are replaced by the fuzzer's plan. Runs only inside Vitest
// (see fast.mjs, which copies this file next to the generated tests).
import { captureArtifact } from '@solidjs/diagnostics';
import { OBSERVE, flush } from 'solid-js';
import { render } from '@solidjs/web';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { appFrames } from './frames.mjs';
import { installVirtualClock } from './virtual-clock.mjs';

// `--virtual-clock` (FUZZ_VIRTUAL_CLOCK=1): the program's timers and the runner's own waits run on a virtual clock (virtual-clock.mjs),
// so a 60 ms wait costs no time. The waits are the same lengths in virtual time, so what a timer can do before the next event is the same.
let clock: ReturnType<typeof installVirtualClock> | null = null;
const pause = (ms: number) => (clock ? clock.advance(ms) : new Promise<void>(resolve => setTimeout(resolve, ms)));
const describeError = (e: any) => ({
  message: String(e?.message ?? e),
  stack: String(e?.stack ?? ''),
  cause: e?.cause ? { message: String(e.cause?.message ?? e.cause), stack: String(e.cause?.stack ?? '') } : null,
});

function events() {
  return [
    ...['click', 'dblclick', 'mousedown', 'mouseup', 'mouseover', 'mouseout', 'pointerdown', 'pointerup', 'contextmenu']
      .map(type => [type, () => new MouseEvent(type, { bubbles: true, cancelable: true })] as const),
    ...['keydown', 'keyup'].map(type => [type, () => new KeyboardEvent(type, { bubbles: true, key: 'a' })] as const),
    ...['focus', 'blur', 'focusin', 'focusout'].map(type => [type, () => new FocusEvent(type, { bubbles: true })] as const),
    ...['input', 'change', 'submit', 'scroll', 'wheel'].map(type => [type, () => new Event(type, { bubbles: true, cancelable: true })] as const),
  ];
}

// The hit ledger (scripts/fuzz/hits.mjs): the instrumented twin counts on globalThis and mirrors the counts into #sc-hits.
// Polls until the counters stay unchanged over two 50 ms ticks (2 s at most): the page is still, or it is not.
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

export async function runCase(id: string, caseDir: string, load: () => Promise<any>, outDirOverride?: string) {
  const started = performance.now();
  const outDir = (outDirOverride ?? process.env.FUZZ_FAST_OUT) as string;
  const settings = { eventPasses: 2, pauseMs: Number(process.env.FUZZ_PAUSE_MS ?? 60), finalMs: Number(process.env.FUZZ_FINAL_MS ?? 150) };
  const diagnostics: any[] = [], errors: any[] = [], consoleErrors: string[] = [], disposers: (() => void)[] = [];
  let witnesses: any[] = [], failure: any = null, phase = 'import', artifactCount: number | null = null;
  let hits: Record<string, number> | null = null, order: number[] = [], hitsDom: string | null = null, quiescent: boolean | null = null;
  (globalThis as any).__scHits = undefined; (globalThis as any).__scOrder = undefined;
  Error.stackTraceLimit = 80;

  const onError = (event: ErrorEvent) => {
    errors.push({ phase, kind: 'window-error', message: String(event.message), stack: String(event.error?.stack ?? '') });
    event.preventDefault();
  };
  const onRejection = (event: PromiseRejectionEvent) => {
    errors.push({ phase, kind: 'unhandledrejection', ...describeError(event.reason) });
    event.preventDefault();
  };
  const onProcessError = (e: any) => errors.push({ phase, kind: 'node-uncaught', ...describeError(e) });
  // A timer callback that throws is an uncaught exception in Node; the virtual clock reports it the same way.
  if (process.env.FUZZ_VIRTUAL_CLOCK === '1') clock = installVirtualClock({ onError: onProcessError });
  const onProcessRejection = (e: any) => errors.push({ phase, kind: 'node-unhandled-rejection', ...describeError(e) });
  process.on('uncaughtException', onProcessError);
  process.on('unhandledRejection', onProcessRejection);
  const oldError = console.error, oldWarn = console.warn;
  console.error = (...args) => { consoleErrors.push(args.map(x => String(x)).join(' ')); };
  console.warn = () => {};
  window.addEventListener('error', onError);
  window.addEventListener('unhandledrejection', onRejection);
  const stop = OBSERVE!.diagnostics.subscribe((event: any) => {
    diagnostics.push({ code: event.code, message: event.message ?? null, loc: event.loc ?? null, siteKind: event.siteKind ?? null,
      phase, stack: new Error(`diagnostic ${event.code}`).stack });
  });
  try {
    const plan = JSON.parse(readFileSync(join(caseDir, 'sites.json'), 'utf8'));
    const captured = await captureArtifact(async () => {
      const mod = await load();
      const root = document.createElement('div'); root.id = 'root'; document.body.append(root);
      phase = 'render';
      // As scripts/runtime-oracle.mjs: every exported capitalized function renders in its own root.
      for (const [name, value] of Object.entries(mod).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
        if (typeof value !== 'function' || !(name === 'default' || /^[A-Z]/.test(name))) continue;
        const Component = value as any;
        const host = document.createElement('section'); host.dataset.export = name; root.append(host);
        try { disposers.push(render(() => <Component />, host)); }
        catch (e) { errors.push({ phase, kind: 'render', export: name, ...describeError(e) }); }
      }
      phase = 'exercise';
      await pause(settings.pauseMs);
      for (let pass = 0; pass < settings.eventPasses; pass++) {
        for (const element of Array.from(root.querySelectorAll('*'))) {
          for (const [type, make] of events()) {
            try { element.dispatchEvent(make()); } catch (e) { errors.push({ phase, kind: 'handler', event: type, ...describeError(e) }); }
          }
        }
        await pause(settings.pauseMs);
      }
      await pause(settings.finalMs);
      phase = 'plan';
      for (const buttonId of plan.plan.clicks) {
        const target = document.querySelector(`[data-act="${buttonId}"]`);
        if (target instanceof HTMLElement) { target.click(); await flush(); await pause(settings.pauseMs); }
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
    process.off('uncaughtException', onProcessError);
    process.off('unhandledRejection', onProcessRejection);
    console.error = oldError; console.warn = oldWarn;
    const record = {
      id, wallMs: Math.round(performance.now() - started), failure, witnesses, hits, order, hitsDom, quiescent,
      diagnostics: diagnostics.map(d => ({ code: d.code, message: d.message, loc: d.loc, siteKind: d.siteKind, phase: d.phase, positions: appFrames(id, d.stack) })),
      errors: errors.filter(e => e.phase !== 'dispose').map(e => ({ phase: e.phase, kind: e.kind, message: e.message, cause: e.cause?.message ?? null,
        positions: [...appFrames(id, e.stack), ...appFrames(id, e.cause?.stack)] })),
      consoleErrors: consoleErrors.slice(0, 20), artifactCount,
    };
    mkdirSync(outDir, { recursive: true });
    writeFileSync(join(outDir, `${id}.json`), JSON.stringify(record));
  }
}
