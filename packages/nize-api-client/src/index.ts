// @zen-component: AUTH-ClientAPI

/**
 * @six5536/nize-api-client
 *
 * Typed API client using openapi-typescript generated types.
 * Talks directly to the Nize API sidecar via fetch.
 */

import type { AuthStatusResponse, LoginRequest, LogoutRequest, LogoutResponse, RefreshRequest, RegisterRequest, TokenResponse, HelloWorldResponse, CreateMcpTokenRequest, CreateMcpTokenResponse, McpTokenListResponse } from "@six5536/nize-api-types";

// Re-export types for convenience
export type { AuthStatusResponse, LoginRequest, LogoutRequest, LogoutResponse, RefreshRequest, RegisterRequest, TokenResponse, HelloWorldResponse, CreateMcpTokenRequest, CreateMcpTokenResponse, McpTokenInfo, McpTokenListResponse } from "@six5536/nize-api-types";

// ============================================================================
// Configuration
// ============================================================================

export interface ApiClientConfig {
  baseUrl: string;
  getToken?: () => string | null | Promise<string | null>;
}

// ============================================================================
// Errors
// ============================================================================

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: unknown,
  ) {
    super(`API Error: ${status}`);
    this.name = "ApiError";
  }
}

// ============================================================================
// API Client
// ============================================================================

/**
 * Typed API client for Nize backend.
 * Uses fetch directly — no Tauri invoke needed.
 */
// @zen-impl: AUTH-1_AC-1, AUTH-3_AC-1, AUTH-4_AC-1
export class NizeApiClient {
  constructor(private config: ApiClientConfig) {}

  // ---------------------------------------------------------------------------
  // Internal helpers
  // ---------------------------------------------------------------------------

  private async request<T>(method: string, path: string, options: { body?: unknown; signal?: AbortSignal } = {}): Promise<T> {
    const url = new URL(path, this.config.baseUrl);

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    const token = await this.config.getToken?.();
    if (token) {
      headers["Authorization"] = `Bearer ${token}`;
    }

    const response = await fetch(url.toString(), {
      method,
      headers,
      body: options.body ? JSON.stringify(options.body) : undefined,
      signal: options.signal,
    });

    if (!response.ok) {
      throw new ApiError(response.status, await response.json().catch(() => null));
    }

    return response.json();
  }

  // ---------------------------------------------------------------------------
  // Authentication
  // ---------------------------------------------------------------------------

  /** Login with email and password. */
  async login(body: LoginRequest): Promise<TokenResponse> {
    return this.request<TokenResponse>("POST", "/auth/login", { body });
  }

  /** Register a new user account. */
  async register(body: RegisterRequest): Promise<TokenResponse> {
    return this.request<TokenResponse>("POST", "/auth/register", { body });
  }

  /** Refresh an access token using a refresh token. */
  async refresh(body: RefreshRequest, options?: { signal?: AbortSignal }): Promise<TokenResponse> {
    return this.request<TokenResponse>("POST", "/auth/refresh", { body, signal: options?.signal });
  }

  /** Logout and revoke a refresh token. */
  async logout(body?: LogoutRequest): Promise<LogoutResponse> {
    return this.request<LogoutResponse>("POST", "/auth/logout", {
      body: body ?? {},
    });
  }

  /** Check whether an admin user exists (for first-run detection). */
  async authStatus(options?: { signal?: AbortSignal }): Promise<AuthStatusResponse> {
    return this.request<AuthStatusResponse>("GET", "/auth/status", { signal: options?.signal });
  }

  // ---------------------------------------------------------------------------
  // Health
  // ---------------------------------------------------------------------------

  /** Bootstrap health check. */
  async hello(): Promise<HelloWorldResponse> {
    return this.request<HelloWorldResponse>("GET", "/api/hello");
  }

  // ---------------------------------------------------------------------------
  // MCP Tokens
  // ---------------------------------------------------------------------------

  /** Create an MCP API token. Requires authentication. */
  async createMcpToken(body: CreateMcpTokenRequest): Promise<CreateMcpTokenResponse> {
    return this.request<CreateMcpTokenResponse>("POST", "/auth/mcp-tokens", { body });
  }

  /** List MCP API tokens for the authenticated user. */
  async listMcpTokens(): Promise<McpTokenListResponse> {
    return this.request<McpTokenListResponse>("GET", "/auth/mcp-tokens");
  }

  /** Revoke an MCP API token. */
  async revokeMcpToken(id: string): Promise<void> {
    await this.request<unknown>("DELETE", `/auth/mcp-tokens/${id}`);
  }
}

export default NizeApiClient;
