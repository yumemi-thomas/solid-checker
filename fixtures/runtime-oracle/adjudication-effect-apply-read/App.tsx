// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json): reads
// in a createEffect apply callback. rc.17 runEffect opens an "an effect callback"
// strict-read window around the apply (@solidjs/signals dist/dev.js:1993), so
// each read warns STRICT_READ_UNTRACKED when the apply runs.
import { createEffect, createSignal } from "solid-js";

// probus-hk data/traffic.ts:54-73: a module signal a helper reads so that its
// callers re-run when the data lands.
let shapes: string[] | null = null;
const [loaded, setLoaded] = createSignal(0);
function trafficShapes(): string[] | null {
  loaded();
  return shapes;
}
function land() {
  shapes = ["a", "b"];
  setLoaded((n) => n + 1);
}

// probus-hk RailDiagram.tsx:349, Search.tsx:334: a direct read in the apply.
export function DirectRead() {
  const [ready, setReady] = createSignal(1);
  const [placed, setPlaced] = createSignal(false);
  createEffect(
    () => ready(),
    (r) => {
      if (!r || placed()) return;
      setPlaced(true);
    },
  );
  return <button type="button" onClick={() => setReady((n) => n + 1)}>{String(placed())}</button>;
}

// probus-hk Plan.tsx:526, VirtualRows.tsx:121: a local helper the apply calls
// reads a signal.
export function HelperRead() {
  const [search, setSearch] = createSignal("from=me");
  const [from, setFrom] = createSignal("");
  const endOf = () => search().split("=")[1];
  createEffect(
    () => search(),
    () => {
      const start = endOf();
      if (start !== from()) setFrom(start);
    },
  );
  return <button type="button" onClick={() => setSearch("from=x")}>{from()}</button>;
}

// probus-hk RouteMap.tsx:1333: the apply reads the helper, so the effect never
// subscribes to `loaded` and the layer stays empty when the shapes land.
export function LostSubscription() {
  const [map] = createSignal("instance");
  let layer!: HTMLParagraphElement;
  createEffect(
    () => map(),
    () => {
      const links = trafficShapes();
      layer.textContent = links ? `drawn ${links.length}` : "empty";
    },
  );
  return (
    <section>
      <button type="button" data-probe="land" onClick={land}>land</button>
      <p data-probe="layer" ref={layer}>pending</p>
    </section>
  );
}

// probus-hk GroupSheet.tsx:50 (Starred.tsx:1006 passes current={ask().current}):
// a prop the caller passes a signal for, read in the apply.
function Sheet(props: { open: boolean; current: string }) {
  const [picked, setPicked] = createSignal("");
  createEffect(
    () => props.open,
    (open) => {
      if (!open) return;
      setPicked(props.current);
    },
  );
  return <p>{picked()}</p>;
}
export function PropRead() {
  const [open, setOpen] = createSignal(true);
  const [ask] = createSignal({ current: "home" });
  return (
    <section>
      <button type="button" onClick={() => setOpen((o) => !o)}>toggle</button>
      <Sheet open={open()} current={ask().current} />
    </section>
  );
}
