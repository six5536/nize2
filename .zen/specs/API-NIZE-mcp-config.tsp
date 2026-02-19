/**
 * MCP server configuration API contract for Nize.
 * Defines endpoints for managing external MCP server connections.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/mcp-config.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi;

// ============================================================================
// Enums
// ============================================================================

enum ServerVisibility {
  hidden,
  visible,
  user,
}

enum ServerTransport {
  stdio,
  http,
  sse,
  managedSse: "managed-sse",
  managedHttp: "managed-http",
}

enum AuthType {
  none,
  apiKey: "api-key",
  oauth,
}

enum ServerStatus {
  enabled,
  disabled,
  unavailable,
  unauthorized,
}

// ============================================================================
// Server Models
// ============================================================================

model MCPServerBase {
  id: UUID;
  name: string;

  @maxLength(500)
  description: string;

  domain: string;
  status: ServerStatus;
  toolCount: int32;
  createdAt: DateTime;
  updatedAt: DateTime;
}

model UserServerView extends MCPServerBase {
  visibility: "visible" | "user";
  isOwned: boolean;
}

model AdminServerView extends MCPServerBase {
  visibility: ServerVisibility;
  transport: ServerTransport;
  authType: AuthType;
  ownerId?: UUID;
  userPreferenceCount: int32;
  enabled: boolean;
  available: boolean;
}

model ServerToolSummary {
  name: string;
  description: string;
}

// ============================================================================
// Request/Response Models
// ============================================================================

model UserServerListResponse {
  servers: UserServerView[];
}

model AdminServerListResponse {
  servers: AdminServerView[];
}

model ServerToolsResponse {
  serverId: UUID;
  tools: ServerToolSummary[];
}

model CreateUserServerRequest {
  name: string;

  @maxLength(500)
  description?: string;

  domain: string;
  url: string;
  headers?: Record<string>;
  authType: AuthType;
  apiKey?: string;
  apiKeyHeader?: string;
}

model UpdateUserServerRequest {
  name?: string;

  @maxLength(500)
  description?: string;

  domain?: string;
  url?: string;
  headers?: Record<string>;
  authType?: AuthType;
  apiKey?: string;
  apiKeyHeader?: string;
}

model CreateBuiltInServerRequest {
  name: string;

  @maxLength(500)
  description?: string;

  domain: string;
  visibility: "hidden" | "visible";
  transport: ServerTransport;
  command?: string;
  args?: string[];
  env?: Record<string>;
  url?: string;
  headers?: Record<string>;
  authType?: AuthType;
  apiKey?: string;
  apiKeyHeader?: string;
}

model UpdateBuiltInServerRequest {
  name?: string;

  @maxLength(500)
  description?: string;

  domain?: string;
  visibility?: "hidden" | "visible";
  enabled?: boolean;
  command?: string;
  args?: string[];
  env?: Record<string>;
  url?: string;
  headers?: Record<string>;
  authType?: AuthType;
  apiKey?: string;
  apiKeyHeader?: string;
}

model TogglePreferenceRequest {
  enabled: boolean;
}

model TestConnectionRequest {
  transport: ServerTransport;
  command?: string;
  args?: string[];
  env?: Record<string>;
  url?: string;
  headers?: Record<string>;
  authType?: AuthType;
  apiKey?: string;
  apiKeyHeader?: string;
}

model TestConnectionResponse {
  success: boolean;
  serverName?: string;
  serverVersion?: string;
  protocolVersion?: string;
  toolCount?: int32;
  error?: string;
  errorDetails?: string;
}

model DeleteServerResponse {
  deleted: boolean;
  warning?: string;
  affectedUsers?: int32;
}

// ============================================================================
// OAuth Models
// ============================================================================

model InitiateOAuthResponse {
  authorizationUrl: string;
}

model OAuthStatusResponse {
  authorized: boolean;
  scopes?: string[];
  expiresAt?: DateTime;
}

// ============================================================================
// Routes
// ============================================================================

@route("/mcp")
@tag("MCP Configuration")
interface MCPConfigRoutes {
  // ========== User Endpoints ==========

  @route("/servers")
  @get
  @summary("List servers for current user")
  listUserServers(): UserServerListResponse | UnauthorizedError;

  @route("/servers")
  @post
  @summary("Add user server")
  addUserServer(
    @body body: CreateUserServerRequest,
  ): AdminServerView | UnauthorizedError;

  @route("/servers/{serverId}")
  @patch
  @summary("Update user server")
  updateUserServer(@path serverId: UUID, @body body: UpdateUserServerRequest):
    | AdminServerView
    | NotFoundError
    | ForbiddenError
    | UnauthorizedError;

  @route("/servers/{serverId}")
  @delete
  @summary("Delete user server")
  deleteUserServer(@path serverId: UUID):
    | void
    | NotFoundError
    | ForbiddenError
    | UnauthorizedError;

  @route("/servers/{serverId}/preference")
  @patch
  @summary("Toggle server preference")
  togglePreference(
    @path serverId: UUID,
    @body body: TogglePreferenceRequest,
  ): void | NotFoundError | UnauthorizedError;

  @route("/servers/{serverId}/tools")
  @get
  @summary("Get server tools")
  getServerTools(
    @path serverId: UUID,
  ): ServerToolsResponse | NotFoundError | UnauthorizedError;

  @route("/servers/{serverId}/oauth/status")
  @get
  @summary("Get OAuth status")
  getOAuthStatus(
    @path serverId: UUID,
  ): OAuthStatusResponse | NotFoundError | UnauthorizedError;

  @route("/servers/{serverId}/oauth/initiate")
  @post
  @summary("Initiate OAuth flow")
  initiateOAuth(
    @path serverId: UUID,
  ): InitiateOAuthResponse | NotFoundError | UnauthorizedError;

  @route("/servers/{serverId}/oauth/revoke")
  @post
  @summary("Revoke OAuth")
  revokeOAuth(@path serverId: UUID): void | NotFoundError | UnauthorizedError;

  @route("/test-connection")
  @post
  @summary("Test connection")
  testConnection(
    @body body: TestConnectionRequest,
  ): TestConnectionResponse | UnauthorizedError;

  // ========== Admin Endpoints ==========

  @route("/admin/servers")
  @get
  @summary("List all servers (admin)")
  listAllServers(
  ): AdminServerListResponse | UnauthorizedError | ForbiddenError;

  @route("/admin/servers")
  @post
  @summary("Create built-in server")
  createBuiltInServer(
    @body body: CreateBuiltInServerRequest,
  ): AdminServerView | UnauthorizedError | ForbiddenError;

  @route("/admin/servers/{serverId}")
  @patch
  @summary("Update built-in server")
  updateBuiltInServer(
    @path serverId: UUID,
    @body body: UpdateBuiltInServerRequest,
  ): AdminServerView | NotFoundError | UnauthorizedError | ForbiddenError;

  @route("/admin/servers/{serverId}")
  @delete
  @summary("Delete built-in server")
  deleteBuiltInServer(@path serverId: UUID):
    | DeleteServerResponse
    | NotFoundError
    | UnauthorizedError
    | ForbiddenError;
}
