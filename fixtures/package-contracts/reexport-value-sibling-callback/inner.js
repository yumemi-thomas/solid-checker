// `subscribe` forwards its caller-supplied callback into an unresolved member
// dispatch (`client.getThing`), so its callback execution is uncertifiable and
// the analyzer records a contract-generation obligation whose subject is
// `subscribe`. `PREFIX` is a plain string constant published from the same
// module.
export function subscribe(client, cb) {
  return client.getThing(cb);
}

export const PREFIX = "reexport-value-sibling";
