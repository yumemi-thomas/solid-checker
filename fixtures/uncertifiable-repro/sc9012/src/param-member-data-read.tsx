// probus-hk routes/Starred.tsx:503 (lib/geo.ts:11): a memo sorts by a distance a helper computes from two
// project values; the helper reads number fields of them and hands those on, which the checker takes for a
// possible call of the member.
import { createMemo, createSignal, type Accessor } from "solid-js";

type LatLng = { lat: number; lng: number };
type Resolved = { id: string; stop?: { location: LatLng } };

const toRad = (degrees: number) => (degrees * Math.PI) / 180;

function distanceM(a: LatLng, b: LatLng): number {
  return Math.hypot(toRad(b.lat - a.lat), toRad(a.lat), a.lng - b.lng);
}

function useGeolocation(): { position: Accessor<LatLng | null> } {
  const [position] = createSignal<LatLng | null>(null);
  return { position };
}

export function Nearest(props: { entries: Resolved[] }) {
  const { position } = useGeolocation();
  const [order] = createSignal<"distance" | "id">("distance");
  const metresTo = (entry: Resolved) => {
    const here = position();
    return here && entry.stop ? distanceM(here, entry.stop.location) : null;
  };
  const sort = (list: Resolved[]): Resolved[] => {
    if (order() === "distance") return [...list].sort((a, b) => (metresTo(a) ?? 0) - (metresTo(b) ?? 0));
    return list;
  };
  const sorted = createMemo(() => sort(props.entries));
  return <p>{sorted().length}</p>;
}
