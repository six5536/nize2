/**
 * @six5536/nize-api-types
 *
 * Generated types from TypeSpec definitions via openapi-typescript.
 * DO NOT EDIT generated files directly. Run `npm run generate` to regenerate.
 */

// Re-export all generated types
export type { paths, components, operations } from "./generated/api.d.ts";

// Convenience type aliases
import type { components } from "./generated/api.d.ts";

// Auth types
export type AuthStatusResponse = components["schemas"]["Auth.AuthStatusResponse"];
export type AuthUser = components["schemas"]["Auth.AuthUser"];
export type LoginRequest = components["schemas"]["Auth.LoginRequest"];
export type RegisterRequest = components["schemas"]["Auth.RegisterRequest"];
export type RefreshRequest = components["schemas"]["Auth.RefreshRequest"];
export type LogoutRequest = components["schemas"]["Auth.LogoutRequest"];
export type TokenResponse = components["schemas"]["Auth.TokenResponse"];
export type LogoutResponse = components["schemas"]["Auth.LogoutResponse"];

// Error types
export type ErrorResponse = components["schemas"]["ErrorResponse"];
export type UnauthorizedError = components["schemas"]["UnauthorizedError"];
export type ValidationError = components["schemas"]["ValidationError"];

// Hello types
export type HelloWorldResponse = components["schemas"]["HelloWorldResponse"];
