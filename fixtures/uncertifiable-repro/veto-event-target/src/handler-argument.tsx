// app-game paint navigation-popup.tsx:68 (zoom.move), grease-pencil-typegpu App.tsx:148, solid-svg-editor
// EditorModals.tsx:425: handlers, on an element or through a project component's prop, hand an object literal to
// a project function that hands it on to a member of another object. The handlers are placed only while no
// dispatch site of the project withholds that proof (ADR 0356), here dispatch-sites.ts's.
type Point = { x: number; y: number };

function createGesture() {
  const history: Point[] = [];
  return {
    record(point: Point) {
      history.push(point);
    },
  };
}

const gesture = createGesture();

function track(point: Point): void {
  gesture.record(point);
}

function Button(props: { onPress: () => void }) {
  return <button onClick={() => props.onPress()}>reset</button>;
}

export function Pad(props: { top: number }) {
  return (
    <div
      onPointerMove={(event: PointerEvent) => track({ x: event.clientX, y: event.clientY })}
      onPointerDown={(event: PointerEvent) => gesture.record({ x: event.clientX, y: event.clientY })}
    >
      <Button onPress={() => track({ x: 0, y: props.top })} />
    </div>
  );
}
