import { configureServerFunctionsClient } from "@solidjs/web/server-functions";
import { saveEvent } from "./api";

declare const options: {
  serializeArgs?: (args: unknown[]) => string;
};

// The exact configuration API is known, but this runtime object may or may not
// install serializeArgs. The Date call is therefore SC7007 uncertifiable.
configureServerFunctionsClient(options);
saveEvent(new Date());
