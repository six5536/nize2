/**
 * Authentication API contract for Nize.
 * Defines JWT-based auth endpoints for login, register, refresh, and logout.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/auth.tsp
 * @zen-component: AUTH-AuthRoutes
 * @zen-impl: AUTH-1_AC-1, AUTH-1_AC-2, AUTH-1_AC-3
 * @zen-impl: AUTH-1.1_AC-1, AUTH-1.1_AC-2, AUTH-1.1_AC-4
 * @zen-impl: AUTH-3_AC-1, AUTH-3_AC-2, AUTH-3_AC-3
 * @zen-impl: AUTH-4_AC-1, AUTH-4_AC-2
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi.Auth;

// ============================================================================
// Request Models
// ============================================================================

/** Login with email and password */
model LoginRequest {
  @doc("User email address")
  @format("email")
  email: string;

  @doc("User password")
  @minLength(1)
  password: string;
}

/** Register a new user account */
model RegisterRequest {
  @doc("User email address")
  @format("email")
  email: string;

  @doc("User password (minimum 8 characters)")
  @minLength(8)
  password: string;

  @doc("User display name")
  name?: string;
}

/** Refresh an access token */
model RefreshRequest {
  @doc("Refresh token to exchange for new token pair (optional when sent via cookie)")
  refreshToken?: string;
}

/** Logout request */
model LogoutRequest {
  @doc("Refresh token to revoke")
  refreshToken?: string;
}

// ============================================================================
// Response Models
// ============================================================================

/** User information returned with auth responses */
model AuthUser {
  @doc("User ID")
  id: string;

  @doc("User email address")
  email: string;

  @doc("User display name")
  name?: string;

  @doc("User roles (e.g., 'admin')")
  roles: string[];
}

/** Token pair response */
model TokenResponse {
  @doc("JWT access token for API authorization")
  accessToken: string;

  @doc("Refresh token for obtaining new access tokens")
  refreshToken: string;

  @doc("Access token expiry in seconds")
  expiresIn: int32;

  @doc("Token type (always 'Bearer')")
  tokenType: "Bearer";

  @doc("Authenticated user information")
  user: AuthUser;
}

/** Successful logout response */
model LogoutResponse {
  @doc("Logout confirmation")
  success: true;
}

/** Auth status response (for first-run detection) */
model AuthStatusResponse {
  @doc("Whether an admin user has been created")
  adminExists: boolean;
}

// ============================================================================
// MCP Token Models
// ============================================================================

/** Request to create an MCP API token */
model CreateMcpTokenRequest {
  @doc("Human-readable name for the token")
  name: string;

  @doc("If true, revoke any existing active token with the same name before creating. If false (default), reject with 409 if a token with the same name already exists.")
  overwrite?: boolean;
}

/** Response after creating an MCP API token (contains plaintext token once) */
model CreateMcpTokenResponse {
  @doc("Token ID")
  id: string;

  @doc("Plaintext token (shown only once)")
  token: string;

  @doc("Token name")
  name: string;

  @doc("Creation timestamp")
  createdAt: string;
}

/** An MCP API token record (without sensitive data) */
model McpTokenInfo {
  @doc("Token ID")
  id: string;

  @doc("Token name")
  name: string;

  @doc("Creation timestamp")
  createdAt: string;

  @doc("Expiry timestamp (if set)")
  expiresAt?: string;

  @doc("Revocation timestamp (if revoked)")
  revokedAt?: string;
}

/** List of MCP tokens */
model McpTokenListResponse {
  @doc("List of tokens")
  tokens: McpTokenInfo[];
}

/** Generic success response */
model SuccessResponse {
  @doc("Operation result")
  success: true;
}

// ============================================================================
// Auth Routes
// ============================================================================

@route("/auth")
@tag("Authentication")
interface AuthRoutes {
  /**
   * Login with email and password.
   * Returns access and refresh token pair on success.
   */
  @post
  @route("/login")
  @summary("Login with credentials")
  login(
    @body body: LoginRequest,
  ): TokenResponse | NizeApi.UnauthorizedError | NizeApi.ValidationError;

  /**
   * Register a new user account.
   * Returns access and refresh token pair on success.
   * First registered user is granted admin role.
   */
  @post
  @route("/register")
  @summary("Register new account")
  register(@body body: RegisterRequest): {
    @statusCode statusCode: 201;
    @body body: TokenResponse;
  } | NizeApi.ValidationError;

  /**
   * Refresh an expired access token.
   * Rotates the refresh token on each use.
   */
  @post
  @route("/refresh")
  @summary("Refresh access token")
  refresh(
    @body body: RefreshRequest,
  ): TokenResponse | NizeApi.UnauthorizedError;

  /**
   * Logout and revoke refresh token.
   * Requires valid access token.
   */
  @post
  @route("/logout")
  @summary("Logout current session")
  logout(@body body: LogoutRequest): LogoutResponse | NizeApi.UnauthorizedError;

  /**
   * Logout from all sessions.
   * Revokes all refresh tokens for the user.
   */
  @post
  @route("/logout/all")
  @summary("Logout all sessions")
  logoutAll(): LogoutResponse | NizeApi.UnauthorizedError;

  /**
   * Handle OAuth callback from MCP server authorization.
   */
  @get
  @route("/oauth/mcp/callback")
  @summary("OAuth callback")
  oauthCallback(
    @query code: string,
    @query state: string,
  ): void | NizeApi.ValidationError;

  /**
   * Check authentication status.
   * Returns whether an admin user exists (for first-run flow).
   */
  @get
  @route("/status")
  @summary("Check auth status")
  status(): AuthStatusResponse;

  /**
   * Create an MCP API token.
   * Requires authentication. Returns plaintext token once.
   */
  @post
  @route("/mcp-tokens")
  @summary("Create MCP token")
  createMcpToken(@body body: CreateMcpTokenRequest): {
    @statusCode statusCode: 201;
    @body body: CreateMcpTokenResponse;
  } | NizeApi.UnauthorizedError;

  /**
   * List MCP API tokens for the authenticated user.
   * Requires authentication. Does not return plaintext tokens.
   */
  @get
  @route("/mcp-tokens")
  @summary("List MCP tokens")
  listMcpTokens(): McpTokenListResponse | NizeApi.UnauthorizedError;

  /**
   * Revoke an MCP API token.
   * Requires authentication.
   */
  @delete
  @route("/mcp-tokens/{id}")
  @summary("Revoke MCP token")
  revokeMcpToken(@path id: string): SuccessResponse | NizeApi.UnauthorizedError;
}
