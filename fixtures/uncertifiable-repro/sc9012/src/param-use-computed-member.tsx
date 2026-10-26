// app-game math-examples/camera-projection-svg/camera-projection-svg.tsx:25: a component hands a vector to a
// project function that reads its elements by a computed index and hands them on.
type Vec4 = [number, number, number, number];

function validate(vector: Vec4): Vec4 {
  const out: Vec4 = [0, 0, 0, 0];
  for (let index = 0; index < 4; index++) out[index] = Math.round(vector[index]!);
  return out;
}

export function Projection() {
  const point: Vec4 = [1, 2, 3, 1];
  const clip = validate(point);
  return <p>{clip.join(",")}</p>;
}
