import { createSignal, For } from "solid-js";

// ---- Real defects the execution-role precision work must keep reporting
// (found in the 2026-10 development-feedback sweep).

// queue-management-ui `Button`: a prop read once in the component body, while
// the parent drives it from a signal -- the filter buttons never restyle.
function Button(props: { variant?: string; onPress?: () => void }) {
  const variant = props.variant ?? "primary";
  return <button class={variant} onClick={props.onPress}>x</button>;
}
export function Filters() {
  const [filter, setFilter] = createSignal("all");
  return (
    <Button
      variant={filter() === "all" ? "primary" : "outline"}
      onPress={() => setFilter("other")}
    />
  );
}

// probus-hk `StopRow`: an event-handler attribute is evaluated once at
// creation, so the row keeps whichever handler `canAlight` selected first.
function StopRow(props: { canAlight: boolean; onAlight: () => void; onBoard: () => void }) {
  return <button onClick={props.canAlight ? props.onAlight : props.onBoard}>go</button>;
}
export function Route(props: { stops: number[]; onAlight: () => void; onBoard: () => void }) {
  const [boardSeq] = createSignal<number | null>(null);
  const [seq] = createSignal(0);
  return (
    <For each={props.stops}>
      {(stop) => (
        <StopRow
          canAlight={boardSeq() !== null && seq() > stop}
          onAlight={props.onAlight}
          onBoard={props.onBoard}
        />
      )}
    </For>
  );
}
