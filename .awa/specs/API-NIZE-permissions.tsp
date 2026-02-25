/**
 * Permission system TypeSpec definitions for Nize API.
 * Defines routes for grants, share links, and admin management.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/permissions.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi;

// ============================================================================
// Permission Types
// ============================================================================

enum PermissionLevel {
  view,
  comment,
  edit,
  full,
}

enum ResourceType {
  conversation,
  document,
  group,
}

// ============================================================================
// Permission Grant Models
// ============================================================================

model PermissionGrant {
  id: UUID;
  granterId: UUID;
  granteeId?: UUID;
  granteeEmail: string;
  resourceType: ResourceType;
  resourceId: UUID;
  level: PermissionLevel;
  cascade: boolean;
  createdAt: DateTime;
}

model CreateGrantRequest {
  email: string;
  level: PermissionLevel;
  cascade?: boolean;
}

model GrantListResponse {
  grants: PermissionGrant[];
}

// ============================================================================
// Share Link Models
// ============================================================================

model ShareLink {
  id: UUID;
  ownerId: UUID;
  resourceType: ResourceType;
  resourceId: UUID;
  token: string;
  level: PermissionLevel;
  cascade: boolean;
  expiresAt?: DateTime;
  createdAt: DateTime;
  url: string;
}

model CreateLinkRequest {
  level: PermissionLevel;
  cascade?: boolean;
  expiresAt?: DateTime;
}

model LinkListResponse {
  links: ShareLink[];
}

model SharedResourceResponse {
  resourceType: ResourceType;
  resourceId: UUID;
  level: PermissionLevel;
  cascade: boolean;
}

// ============================================================================
// Admin Models
// ============================================================================

model SetAdminRoleRequest {
  isAdmin: boolean;
}

// ============================================================================
// Permission Routes
// ============================================================================

@route("/permissions")
@tag("Permissions")
interface PermissionRoutes {
  @post
  @route("/{resourceType}/{resourceId}/grants")
  createGrant(
    @path resourceType: ResourceType,
    @path resourceId: UUID,
    @body body: CreateGrantRequest,
  ): PermissionGrant | ForbiddenError | NotFoundError;

  @get
  @route("/{resourceType}/{resourceId}/grants")
  listGrants(
    @path resourceType: ResourceType,
    @path resourceId: UUID,
  ): GrantListResponse | ForbiddenError | NotFoundError;

  @delete
  @route("/grants/{grantId}")
  revokeGrant(@path grantId: UUID): void | ForbiddenError | NotFoundError;

  @post
  @route("/{resourceType}/{resourceId}/links")
  createLink(
    @path resourceType: ResourceType,
    @path resourceId: UUID,
    @body body: CreateLinkRequest,
  ): ShareLink | ForbiddenError | NotFoundError;

  @get
  @route("/{resourceType}/{resourceId}/links")
  listLinks(
    @path resourceType: ResourceType,
    @path resourceId: UUID,
  ): LinkListResponse | ForbiddenError | NotFoundError;

  @delete
  @route("/links/{linkId}")
  revokeLink(@path linkId: UUID): void | ForbiddenError | NotFoundError;

  @get
  @route("/shared/{token}")
  accessShared(@path token: string): SharedResourceResponse | NotFoundError;
}

// ============================================================================
// Admin Permission Routes
// ============================================================================

@route("/admin/permissions")
@tag("Admin")
interface AdminPermissionRoutes {
  @get
  @route("/grants")
  listAllGrants(): GrantListResponse | ForbiddenError;

  @get
  @route("/links")
  listAllLinks(): LinkListResponse | ForbiddenError;

  @get
  @route("/groups")
  listAllGroups(): GrantListResponse | ForbiddenError;

  @delete
  @route("/grants/{grantId}")
  adminRevokeGrant(@path grantId: UUID): void | ForbiddenError | NotFoundError;

  @delete
  @route("/links/{linkId}")
  adminRevokeLink(@path linkId: UUID): void | ForbiddenError | NotFoundError;

  @patch
  @route("/users/{userId}/admin")
  setAdminRole(
    @path userId: UUID,
    @body body: SetAdminRoleRequest,
  ): void | ForbiddenError | NotFoundError;
}
