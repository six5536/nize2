/**
 * Nize API — Main entry point
 *
 * Imports all API modules and serves as the entry point
 * for TypeSpec compilation.
 */
import "./API-NIZE-common.tsp";
import "./API-NIZE-auth.tsp";
import "./API-NIZE-config.tsp";
import "@typespec/http";

using TypeSpec.Http;

namespace NizeApi;

// ============================================================================
// Hello World
// ============================================================================

/** Health check and bootstrap verification response. */
model HelloWorldResponse {
  /** Greeting from nize_core. */
  greeting: string;

  /** Whether the PostgreSQL database is reachable. */
  dbConnected: boolean;

  /** Bun version string, or null if unavailable. */
  bunVersion: string | null;

  /** Whether the Bun sidecar runtime is available. */
  bunAvailable: boolean;
}

@route("/api")
namespace Api {
  @route("/hello")
  @tag("Hello")
  namespace Hello {
    /** Bootstrap health check — verifies core lib, DB, and Bun sidecar. */
    @get
    op helloWorld(): HelloWorldResponse;
  }
}
