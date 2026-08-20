const items = ["a", "b"];

export const App = (props: { ready: boolean }) => (
  <main>
    <ul>{items.map((item, index) => item + index)}</ul>
    {props.ready && <span>ready</span>}
  </main>
);
