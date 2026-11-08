import { render } from "@solidjs/web";
import { CaughtAtFive } from "./caught-at-five";
import { CaughtAtSeven } from "./caught-at-seven";
import { CaughtPastBudget } from "./caught-past-budget";
import { SettledPastBudget } from "./settled-past-budget";
render(() => <><CaughtAtFive /><CaughtAtSeven /><CaughtPastBudget /><SettledPastBudget /></>, document.body);
