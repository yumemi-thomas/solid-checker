import "fz-proto-site-bare-eventtarget";
function Wrap() {
  dispatchEvent(new Event("fzprobe"));
  return null;
}
export function App() {
  return <Wrap />;
}
