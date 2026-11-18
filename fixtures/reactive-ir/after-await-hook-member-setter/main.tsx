import { render } from "@solidjs/web";
import { AsyncRead, Carried, Compared, StoreCarried } from "./App";

render(() => <Carried />, document.body);
render(() => <Compared />, document.body);
render(() => <StoreCarried />, document.body);
render(() => <AsyncRead />, document.body);
