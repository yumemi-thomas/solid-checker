// jandibat-web apps/web/src/pages/AuthPage.tsx:84 (and 13 more calls): a component
// passes a generated request object to a project function that hands it to
// relay-runtime's `getRequest`. The object is a literal the checker can read;
// what the package function does with it is not.
import { getRequest, type ConcreteRequest, type GraphQLTaggedNode } from "relay-runtime";

function createEphemeralMutation(apiBaseUrl: string, mutation: GraphQLTaggedNode) {
  const request = getRequest(mutation);
  return () => fetch(`${apiBaseUrl}/graphql`, { method: "POST", body: request.params.text });
}

const signOutMutation = {
  kind: "Request",
  params: { id: null, metadata: {}, name: "SignOut", operationKind: "mutation", text: "mutation SignOut { signOut }" },
} as unknown as ConcreteRequest;

export function SignOutButton() {
  const signOut = createEphemeralMutation("/api", signOutMutation);
  return <button onClick={() => void signOut()}>out</button>;
}
