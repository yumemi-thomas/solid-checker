// jandibat-web apps/web/src/pages/AuthPage.tsx:190 (and 11 more calls): a project
// function hands its parameter to another that wraps it in the options literal it
// gives, inside a closure, to relay-runtime's `commitMutation` (solid-relay's
// `createMutation`, packages/solid-relay/src/primitives/createMutation.ts:37).
import { commitMutation, type ConcreteRequest, type Environment, type GraphQLTaggedNode } from "relay-runtime";

declare const environment: Environment;

function createMutation(mutation: GraphQLTaggedNode) {
  const commit = (variables: Record<string, unknown>) =>
    commitMutation(environment, { variables, mutation, onCompleted: () => {} });
  return [commit] as const;
}

function createRelayMutation(mutation: GraphQLTaggedNode) {
  const [commit] = createMutation(mutation);
  return (variables: Record<string, unknown>) =>
    new Promise<void>((resolve) => {
      commit(variables);
      resolve();
    });
}

const signOutMutation = {
  kind: "Request",
  params: { id: null, metadata: {}, name: "SignOut", operationKind: "mutation", text: "mutation SignOut { signOut }" },
} as unknown as ConcreteRequest;

export function SignOutButton() {
  const signOut = createRelayMutation(signOutMutation);
  return <button onClick={() => void signOut({})}>out</button>;
}
