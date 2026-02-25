// @awa-component: PLAN-032-StdioConfigFields

/**
 * Stdio transport configuration fields:
 * command, arguments, environment variables.
 */

"use client";

interface EnvPair {
  key: string;
  value: string;
}

interface StdioConfigFieldsProps {
  command: string;
  args: string;
  envPairs: EnvPair[];
  onCommandChange: (command: string) => void;
  onArgsChange: (args: string) => void;
  onEnvPairsChange: (envPairs: EnvPair[]) => void;
}

export function StdioConfigFields({ command, args, envPairs, onCommandChange, onArgsChange, onEnvPairsChange }: StdioConfigFieldsProps) {
  return (
    <>
      <div>
        <label className="block text-sm font-medium text-gray-700">Command</label>
        <input type="text" value={command} onChange={(e) => onCommandChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="npx @modelcontextprotocol/server-filesystem" required />
      </div>
      <div>
        <label className="block text-sm font-medium text-gray-700">Arguments (space-separated)</label>
        <input type="text" value={args} onChange={(e) => onArgsChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="/path/to/allowed/directory" />
      </div>
      <div>
        <label className="block text-sm font-medium text-gray-700">Environment Variables</label>
        <div className="space-y-2 mt-1">
          {envPairs.map((pair, idx) => (
            <div key={idx} className="flex gap-2">
              <input
                type="text"
                value={pair.key}
                onChange={(e) => {
                  const newPairs = [...envPairs];
                  newPairs[idx].key = e.target.value;
                  onEnvPairsChange(newPairs);
                }}
                className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                placeholder="KEY"
              />
              <input
                type="text"
                value={pair.value}
                onChange={(e) => {
                  const newPairs = [...envPairs];
                  newPairs[idx].value = e.target.value;
                  onEnvPairsChange(newPairs);
                }}
                className="flex-1 rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono"
                placeholder="value"
              />
              <button type="button" onClick={() => onEnvPairsChange(envPairs.filter((_, i) => i !== idx))} className="px-2 py-1 text-red-500 hover:text-red-700">
                ✕
              </button>
            </div>
          ))}
          <button type="button" onClick={() => onEnvPairsChange([...envPairs, { key: "", value: "" }])} className="text-sm text-blue-600 hover:text-blue-800">
            + Add environment variable
          </button>
        </div>
      </div>
    </>
  );
}
