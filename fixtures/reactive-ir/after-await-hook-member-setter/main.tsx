import { render } from "@solidjs/web";
import { AsyncRead, Carried, Compared, StoreCarried, StoreDraft } from "./App";

render(() => <Carried />, document.body);
render(() => <Compared />, document.body);
render(() => <StoreDraft />, document.body);
render(() => <StoreCarried />, document.body);
render(() => <AsyncRead />, document.body);
