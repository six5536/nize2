// @zen-component: PLAN-032-OAuthConfigFields

/**
 * OAuth configuration input fields:
 * Client ID, Client Secret, Scopes, and advanced settings
 * (Authorization URL, Token URL).
 */

"use client";

interface OAuthConfigFieldsProps {
  clientId: string;
  clientSecret: string;
  oauthScopes: string;
  authorizationUrl: string;
  tokenUrl: string;
  onClientIdChange: (clientId: string) => void;
  onClientSecretChange: (clientSecret: string) => void;
  onOauthScopesChange: (scopes: string) => void;
  onAuthorizationUrlChange: (url: string) => void;
  onTokenUrlChange: (url: string) => void;
  /** Placeholder for client secret (e.g. "Leave blank to keep existing"). */
  clientSecretPlaceholder?: string;
  /** Whether client secret is required (default: true, set false for edit mode). */
  clientSecretRequired?: boolean;
}

export function OAuthConfigFields({ clientId, clientSecret, oauthScopes, authorizationUrl, tokenUrl, onClientIdChange, onClientSecretChange, onOauthScopesChange, onAuthorizationUrlChange, onTokenUrlChange, clientSecretPlaceholder = "Google OAuth Client Secret", clientSecretRequired = true }: OAuthConfigFieldsProps) {
  return (
    <>
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium text-gray-700">Client ID</label>
          <input type="text" value={clientId} onChange={(e) => onClientIdChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder="Google OAuth Client ID" required />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">Client Secret</label>
          <input type="password" value={clientSecret} onChange={(e) => onClientSecretChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" placeholder={clientSecretPlaceholder} required={clientSecretRequired} />
        </div>
      </div>
      <div>
        <label className="block text-sm font-medium text-gray-700">Scopes</label>
        <input type="text" value={oauthScopes} onChange={(e) => onOauthScopesChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm font-mono" placeholder="openid email profile" />
      </div>
      <details className="text-sm text-gray-500">
        <summary className="cursor-pointer hover:text-gray-700">Advanced OAuth settings</summary>
        <div className="mt-2 space-y-3">
          <div>
            <label className="block text-sm font-medium text-gray-700">Authorization URL</label>
            <input type="url" value={authorizationUrl} onChange={(e) => onAuthorizationUrlChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">Token URL</label>
            <input type="url" value={tokenUrl} onChange={(e) => onTokenUrlChange(e.target.value)} className="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 sm:text-sm" />
          </div>
        </div>
      </details>
    </>
  );
}
