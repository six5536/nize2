// @zen-component: PLAN-032-HttpConfigFields

/**
 * HTTP transport configuration fields:
 * URL, authentication type, API key.
 */

"use client";

import type { AuthType } from "./types";

interface HttpConfigFieldsProps {
  url: string;
  authType: AuthType;
  apiKey: string;
  onUrlChange: (url: string) => void;
  onAuthTypeChange: (authType: AuthType) => void;
  onApiKeyChange: (apiKey: string) => void;
  /** Placeholder text for the API key field (e.g. "Leave blank to keep existing"). */
  apiKeyPlaceholder?: string;
  /** Whether URL is required (true in create mode). */
  urlRequired?: boolean;
}

export function HttpConfigFields({ url, authType, apiKey, onUrlChange, onAuthTypeChange, onApiKeyChange, apiKeyPlaceholder = "Enter API key", urlRequired = true }: HttpConfigFieldsProps) {
  return (
    <>
      <div>
        <label className="block text-sm font-medium text-gray-700">Server URL</label>
        <input type="url" value={url} onChange={(e) => onUrlChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="https://mcp.example.com" required={urlRequired} />
      </div>
      <div>
        <label className="block text-sm font-medium text-gray-700">Authentication</label>
        <select value={authType} onChange={(e) => onAuthTypeChange(e.target.value as AuthType)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm">
          <option value="none">None</option>
          <option value="api-key">API Key</option>
          <option value="oauth">OAuth</option>
        </select>
      </div>
      {authType === "api-key" && (
        <div>
          <label className="block text-sm font-medium text-gray-700">API Key</label>
          <input type="password" value={apiKey} onChange={(e) => onApiKeyChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder={apiKeyPlaceholder} required />
        </div>
      )}
    </>
  );
}
