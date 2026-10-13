// An aliased import: the local name is `config`, the export it reaches is
// `sharedConfig`, one of the five names rc.9's typings do not declare.
import { sharedConfig as config } from "solid-js";

export const shared = config;
