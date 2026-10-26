// inferay AgentChatView/ChatWorkspacePicker.tsx:6-31 (callback read from AgentChatMessages.tsx:48), InlineDirectoryPicker/index.tsx:172 and
// SelectedDirectoryChip.tsx:28: a callback reaches a child that binds it, `onClick={props.onRemove.bind(null, props.path)}`, through one forwarding child.
function Chip(props: { path: string; onRemove: (path: string) => void }) {
  return <button onClick={props.onRemove.bind(null, props.path)}>{props.path}</button>;
}

function Picker(props: { onSelect: (path: string) => void }) {
  return <Chip path="/a" onRemove={(path) => props.onSelect(path)} />;
}

export function Workspace(props: { cwd: string }) {
  return <Picker onSelect={(path) => console.log(props.cwd, path)} />;
}
