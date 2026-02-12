/**
 * Nize API — Main entry point
 *
 * Imports all API modules and serves as the entry point
 * for TypeSpec compilation.
 */
import "./API-NIZE-common.tsp";
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

  /** Node.js version string, or null if unavailable. */
  nodeVersion: string | null;

  /** Whether the Node.js sidecar runtime is available. */
  nodeAvailable: boolean;
}

@route("/api")
namespace Api {
  @route("/hello")
  @tag("Hello")
  namespace Hello {
    /** Bootstrap health check — verifies core lib, DB, and Node sidecar. */
    @get
    op helloWorld(): HelloWorldResponse;
  }
}
