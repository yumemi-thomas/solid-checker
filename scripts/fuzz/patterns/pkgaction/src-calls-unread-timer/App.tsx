import { go } from "fz-src2";
function Wrap() {
  setTimeout(() => {
    go();
  });
  return null;
}
export function App() {
  return <Wrap />;
}
