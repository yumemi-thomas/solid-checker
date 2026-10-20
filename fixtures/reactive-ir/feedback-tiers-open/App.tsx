// A host object the project cannot see into: when it runs a callback is
// unknown.
declare const host: { schedule(callback: () => void): void };

// An exported helper that hands its callback parameter to that host. A
// contract for this export could not say when the callback runs.
export function forward(run: () => void) {
  host.schedule(run);
}

// A helper that invokes a member of a caller-supplied value.
function ageOf(date: Date) {
  return Date.now() - date.getTime();
}

export function Shown(props: { at: Date }) {
  // In the body: the dispatch decides whether this read is untracked.
  const age = ageOf(props.at);
  // In a click handler: no read rule reports what runs there.
  return <button onClick={() => console.log(ageOf(props.at))}>{age}</button>;
}
