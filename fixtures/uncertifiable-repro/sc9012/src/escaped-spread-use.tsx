// kui feature-ksql KsqlRoute.tsx:163: the component is rendered with a spread
// (`<SensorDemo {...props.rest} />`), so its call sites are unknown. It passes an
// options object to a project function that hands it on to a member of another
// object (app-game SensorDemo.tsx:15, solid-dnd dragSensor/solid.ts:103).
type SensorOptions = { threshold?: number; onDragStart?: () => void };

function createSensorScope() {
  return {
    createSensor(options: SensorOptions) {
      return { threshold: options.threshold ?? 4, start: () => options.onDragStart?.() };
    },
  };
}

function createDragSensor(options: SensorOptions = {}) {
  const scope = createSensorScope();
  return scope.createSensor(options);
}

function SensorDemo(props: { label?: string }) {
  const sensor = createDragSensor({ threshold: 8, onDragStart: () => console.log("start") });
  return <div onPointerDown={() => sensor.start()}>{props.label ?? sensor.threshold}</div>;
}

export function Page(props: { rest: { label?: string } }) {
  return <SensorDemo {...props.rest} />;
}
