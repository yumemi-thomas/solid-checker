import { render } from "@solidjs/web";
import "./patch";
import { BodyWrite } from "./body-write";
render(() => <BodyWrite />, document.body);
