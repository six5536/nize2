// @zen-component: NAV-Errors

export class NavigationError extends Error {
  constructor(
    message: string,
    public readonly code: NavigationErrorCode,
  ) {
    super(message);
    this.name = "NavigationError";
  }
}

export type NavigationErrorCode = "CONVERSATION_NOT_FOUND" | "CREATE_FAILED" | "DELETE_FAILED" | "TITLE_GENERATION_FAILED";

export function isNavigationError(error: unknown): error is NavigationError {
  return error instanceof NavigationError;
}
