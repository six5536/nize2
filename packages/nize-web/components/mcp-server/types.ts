// @zen-component: PLAN-032-McpServerTypes

/**
 * Shared types for MCP server form components.
 *
 * Extracted from admin/tools/page.tsx and tools/page.tsx
 * to enable a unified ServerForm component.
 */

export type TransportType = "stdio" | "http";
export type AuthType = "none" | "api-key" | "oauth";
export type VisibilityTier = "hidden" | "visible" | "user";

export interface OAuthConfig {
  clientId: string;
  authorizationUrl: string;
  tokenUrl: string;
  scopes: string[];
}

export interface OAuthStatus {
  connected: boolean;
  expiresAt?: string;
}

export interface StdioConfig {
  transport: "stdio";
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export interface HttpConfig {
  transport: "http";
  url: string;
  authType: AuthType;
  apiKey?: string;
}

export type ServerConfig = StdioConfig | HttpConfig;

export interface TestConnectionResult {
  success: boolean;
  toolCount?: number;
  error?: string;
  authRequired?: boolean;
}

/** Values used to populate the form in edit mode. */
export interface ServerFormValues {
  id: string;
  name: string;
  description: string;
  domain: string;
  visibility: VisibilityTier;
  transport: TransportType;
  authType: AuthType;
  config?: Record<string, unknown>;
  oauthConfig?: OAuthConfig;
}

/** Form submission payload for create / update. */
export interface ServerFormPayload {
  name: string;
  description: string;
  domain: string;
  visibility: "hidden" | "visible";
  config: ServerConfig;
  oauthConfig?: OAuthConfig;
  clientSecret?: string;
}
