// app-game SensorDemo.tsx:15: a component passes an options object to a
// project function that hands it on to a member of another object
// (solid-dnd createDragSensor.tsx:515-523, `factory.createSensor(options)`).
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

export function SensorDemo() {
  const sensor = createDragSensor({ threshold: 8, onDragStart: () => console.log("start") });
  return <div onPointerDown={() => sensor.start()}>{sensor.threshold}</div>;
}
