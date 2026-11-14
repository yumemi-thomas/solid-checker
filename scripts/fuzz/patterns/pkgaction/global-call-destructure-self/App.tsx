import "fz-w";
function Wrap() {
  const { btoa: enc } = window.self;
  enc("x");
  return null;
}
export function App() {
  return <Wrap />;
}
