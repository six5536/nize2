// @zen-component: PLAN-032-McpServerTypes

/**
 * Shared types for MCP server form components.
 *
 * Extracted from admin/tools/page.tsx and tools/page.tsx
 * to enable a unified ServerForm component.
 */

export type TransportType = "stdio" | "http" | "sse" | "managed-sse" | "managed-http";
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

// @zen-impl: PLAN-033 T-XMCP-090 — SSE config type
export interface SseConfig {
  transport: "sse";
  url: string;
  headers?: Record<string, string>;
  authType: AuthType;
  apiKeyHeader?: string;
}

// @zen-impl: PLAN-033 T-XMCP-091 — Managed HTTP/SSE config type
export interface ManagedHttpConfig {
  transport: "managed-sse" | "managed-http";
  command: string;
  args?: string[];
  env?: Record<string, string>;
  port: number;
  path?: string;
  readyTimeoutSecs?: number;
}

export type ServerConfig = StdioConfig | HttpConfig | SseConfig | ManagedHttpConfig;

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
