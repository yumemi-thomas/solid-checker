export const App = (props: { ready: boolean }) => (
  <main>{props.ready && <span>ready</span>}</main>
);
