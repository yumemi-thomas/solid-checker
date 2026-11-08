const log = (...a: any[]) => (window as any).__log(...a);
import "fz-pkg";
export function App() {
  const r = (window as any).__fzRead();
  log("r=" + r);
  return <p>{r}</p>;
}
