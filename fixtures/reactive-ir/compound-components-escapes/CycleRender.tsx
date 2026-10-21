import { Screen } from "./Cycle";

function mount(render: () => unknown) {
  return render();
}

mount(() => <Screen />);
