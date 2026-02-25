/**
 * Configuration API contract for Nize.
 * Supports hierarchical config: system, user-override scopes.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/config.tsp
 * @awa-component: CFG-TypeSpec
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi.Config;

// ============================================================================
// Models
// ============================================================================

/** Validator definition for config values */
model ConfigValidatorModel {
  @doc("Validator type: required, min, max, regex")
  type: string;

  @doc("Validator value (e.g., minimum value, regex pattern)")
  value?: string | int32;

  @doc("Error message when validation fails")
  message?: string;
}

/** Resolved config item — definition metadata + effective value for a user */
model ResolvedConfigItem {
  @doc("Dotted key path (e.g., 'agent.model.temperature')")
  key: string;

  @doc("Category for grouping")
  category: string;

  @doc("Data type of the config value")
  type: string;

  @doc("Display type for UI rendering")
  displayType: string;

  @doc("Possible values for selector display type")
  possibleValues?: string[];

  @doc("Validators for the config value")
  validators?: ConfigValidatorModel[];

  @doc("Default value when no value record exists")
  defaultValue: string;

  @doc("Human-readable label for UI")
  label?: string;

  @doc("Help text for UI")
  description?: string;

  @doc("Current effective value")
  value: string;

  @doc("True if user has an override for this setting")
  isOverridden: boolean;
}

/** Admin config value with scope metadata */
model AdminConfigValueItem {
  @doc("Value record ID")
  id: string;

  @doc("Scope of this value")
  scope: string;

  @doc("User ID if scope is user-override")
  userId?: string;

  @doc("Config value")
  value: string;

  @doc("Last update timestamp")
  updatedAt: string;
}

/** Admin config item — definition + all values across scopes */
model AdminConfigItem {
  @doc("Dotted key path")
  key: string;

  @doc("Category for grouping")
  category: string;

  @doc("Data type")
  type: string;

  @doc("Display type for UI")
  displayType: string;

  @doc("Possible values for selector")
  possibleValues?: string[];

  @doc("Default value")
  defaultValue: string;

  @doc("Human-readable label")
  label?: string;

  @doc("Help text")
  description?: string;

  @doc("All values across scopes")
  values: AdminConfigValueItem[];
}

// ============================================================================
// Request Models
// ============================================================================

/** Update a configuration value */
model UpdateConfigRequest {
  @doc("New value for the configuration item")
  value: string;
}

/** Admin update with optional userId for user-override scope */
model AdminUpdateConfigRequest {
  @doc("New value")
  value: string;

  @doc("User ID when updating user-override scope")
  userId?: string;
}

// ============================================================================
// Response Models
// ============================================================================

/** User config list response */
model UserConfigListResponse {
  @doc("Array of resolved config items with effective values")
  items: ResolvedConfigItem[];
}

/** Admin config list response */
model AdminConfigListResponse {
  @doc("Array of config items with scope information")
  items: AdminConfigItem[];
}

// ============================================================================
// User Config Routes - /config/user
// ============================================================================

@route("/config/user")
@tag("Configuration")
interface UserConfigRoutes {
  /**
   * Get all user configuration items with effective values.
   * Resolves through: user-override → system → definition.defaultValue
   */
  @get
  @summary("Get user config")
  list(): UserConfigListResponse | NizeApi.UnauthorizedError;

  /**
   * Update a user configuration value.
   * Creates or updates user-override for the authenticated user.
   */
  @patch
  @route("/{key}")
  @summary("Update user config")
  update(
    @path key: string,
    @body body: UpdateConfigRequest,
  ): ResolvedConfigItem | NizeApi.UnauthorizedError | NizeApi.ValidationError;

  /**
   * Reset a user configuration to default.
   * Deletes the user-override, falling back to system or definition.defaultValue.
   */
  @delete
  @route("/{key}")
  @summary("Reset user config to default")
  reset(@path key: string): void | NizeApi.UnauthorizedError;
}

// ============================================================================
// Admin Config Routes - /admin/config
// ============================================================================

@route("/admin/config")
@tag("Admin Configuration")
interface AdminConfigRoutes {
  /**
   * Get all configuration items with filtering.
   * Returns definitions with all values across scopes.
   */
  @get
  @summary("Get all config (admin)")
  list(
    @query scope?: string,
    @query userId?: string,
    @query category?: string,
    @query search?: string,
  ): AdminConfigListResponse | NizeApi.UnauthorizedError;

  /**
   * Update a configuration value at a specific scope.
   * For user-override scope, userId must be provided in body.
   */
  @patch
  @route("/{scope}/{key}")
  @summary("Update config at scope (admin)")
  update(
    @path scope: string,
    @path key: string,
    @body body: AdminUpdateConfigRequest,
  ): AdminConfigItem | NizeApi.UnauthorizedError | NizeApi.ValidationError;
}
