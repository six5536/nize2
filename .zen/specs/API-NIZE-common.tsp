/**
 * Common TypeSpec definitions for Nize API.
 * Shared models, decorators, and base configurations.
 */
import "@typespec/http";
import "@typespec/rest";
import "@typespec/openapi";
import "@typespec/openapi3";

using TypeSpec.Http;
using TypeSpec.Rest;
using TypeSpec.OpenAPI;

@service({
  title: "Nize API",
})
@server("http://localhost:3100", "Development server")
namespace NizeApi;

// ============================================================================
// Common Error Models
// ============================================================================

@error
model ErrorResponse {
  @statusCode statusCode: 500;
  error: string;
  message: string;
}
