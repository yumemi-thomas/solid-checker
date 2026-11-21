import { render } from "@solidjs/web";
import { Body, BodyAndHandler, Varies } from "./App";

render(() => <Varies />, document.body);
render(() => <Body />, document.body);
render(() => <BodyAndHandler />, document.body);
