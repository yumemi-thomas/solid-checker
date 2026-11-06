import { render } from "@solidjs/web";
import { App as A10 } from "./a10";
import { App as A12 } from "./a12";
import { App as C15 } from "./c15";
import { App as C16 } from "./c16";
import { App as G1 } from "./g2d-empty";
import { App as G2 } from "./g2d-handler";
import { App as G3 } from "./g2d-under-loading";
import { App as N7 } from "./n7";
import { App as R8 } from "./r8";
import { App as R9 } from "./r9";

const host = document.body;
for (const app of [A10, A12, C15, C16, G1, G2, G3, N7, R8, R9]) render(app, host);
