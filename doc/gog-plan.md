# gogmcp Auth Rewrite: Google OIDC Validation

## Goal

Replace gogmcp's HMAC-based JWT validation with Google OIDC ID token validation. This eliminates the shared `GOGMCP_JWT_SECRET` — gogmcp instead validates tokens signed by Google using Google's public keys, with trust anchored on an OAuth client ID allowlist.

## Current State

### Files to Modify

| File | Current | Target |
|------|---------|--------|
| `internal/auth/jwt.go` | `JWTValidator` with HMAC shared secret | `GoogleIDValidator` with JWKS public key validation |
| `internal/auth/types.go` | Custom `Claims{Email, AuthType, GoogleAccessToken}` | Google OIDC `Claims{Email, EmailVerified, ...}` |
| `internal/auth/context.go` | `ContextFunc` checks `auth_type` field | `ContextFunc` always requires `X-Google-Access-Token` |
| `internal/auth/errors.go` | Current errors | Add `ErrInvalidAudience`, `ErrEmailNotVerified` |
| `internal/config/config.go` | `JWTSecret string` | `AuthMode string`, `AllowedClientIDs []string` |
| `internal/mcp/server.go` | Creates `JWTValidator` from secret | Creates `GoogleIDValidator` or `JWTValidator` based on mode |
| `cmd/testjwt/` | Generates HMAC JWTs | **Repurpose** to generate test tokens or remove |

### Files Unchanged

| File | Why |
|------|-----|
| `internal/google/auth.go` | Token context helpers — unchanged |
| `internal/google/calendar.go` | Calendar service creation — unchanged |
| `internal/mcp/tools.go` | Tool registration + `withGoogleTokenContext` — unchanged |
| `internal/generated/` | Generated tool definitions — unchanged |

## Wire Format (Before → After)

### Before

```
Authorization: Bearer <custom-hmac-jwt>
X-Google-Access-Token: <access_token>    (or google_access_token claim in JWT)
```

Custom JWT payload:
```json
{"email": "user@example.com", "auth_type": "oauth", "iat": ..., "exp": ...}
```

### After

```
Authorization: Bearer <google-id-token>
X-Google-Access-Token: <access_token>
```

Google ID token payload (parsed by gogmcp, signed by Google):
```json
{
  "iss": "https://accounts.google.com",
  "aud": "123456.apps.googleusercontent.com",
  "sub": "1234567890",
  "email": "user@example.com",
  "email_verified": true,
  "iat": 1708300000,
  "exp": 1708303600
}
```

## Implementation Steps

### Step 1: Update Config (`internal/config/config.go`)

**Current:**
```go
type Config struct {
    Port               string
    TLSCertFile        string
    TLSKeyFile         string
    JWTSecret          string
    ServiceAccountFile string
    QuotaProject       string
}
```

**Target:**
```go
type Config struct {
    Port               string
    TLSCertFile        string
    TLSKeyFile         string
    AuthMode           string   // "google" or "jwt"
    AllowedClientIDs   []string // For google mode: allowed OAuth client IDs (aud claim)
    JWTSecret          string   // For jwt mode (legacy): HMAC shared secret
    ServiceAccountFile string
    QuotaProject       string
}
```

**Env vars:**

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `GOGMCP_AUTH_MODE` | No | `"google"` | Auth mode: `"google"` (OIDC) or `"jwt"` (legacy HMAC) |
| `GOGMCP_ALLOWED_CLIENT_IDS` | If mode=google | — | Comma-separated OAuth client IDs |
| `GOGMCP_JWT_SECRET` | If mode=jwt | — | Legacy HMAC secret |

**Implementation detail for `LoadConfig()`:**
```go
authMode := os.Getenv("GOGMCP_AUTH_MODE")
if authMode == "" {
    authMode = "google" // Default to google mode
}

switch authMode {
case "google":
    clientIDsStr := os.Getenv("GOGMCP_ALLOWED_CLIENT_IDS")
    if clientIDsStr == "" {
        return nil, fmt.Errorf("GOGMCP_ALLOWED_CLIENT_IDS is required when GOGMCP_AUTH_MODE=google")
    }
    // Split on comma, trim whitespace
    ids := strings.Split(clientIDsStr, ",")
    for i := range ids {
        ids[i] = strings.TrimSpace(ids[i])
    }
    cfg.AllowedClientIDs = ids

case "jwt":
    jwtSecret := os.Getenv("GOGMCP_JWT_SECRET")
    if jwtSecret == "" {
        return nil, fmt.Errorf("GOGMCP_JWT_SECRET is required when GOGMCP_AUTH_MODE=jwt")
    }
    cfg.JWTSecret = jwtSecret

default:
    return nil, fmt.Errorf("invalid GOGMCP_AUTH_MODE: %s (expected 'google' or 'jwt')", authMode)
}
```

**Update validation** in `LoadConfig()`:
- Remove the unconditional `if jwtSecret == ""` check
- Replace with mode-specific validation as above
- Keep `TLSCertFile` and `TLSKeyFile` as unconditionally required

**Tests to update** in `internal/config/config_test.go`:
- Add test for `GOGMCP_AUTH_MODE=google` with `GOGMCP_ALLOWED_CLIENT_IDS`
- Add test for `GOGMCP_AUTH_MODE=jwt` with `GOGMCP_JWT_SECRET`
- Add test for missing `GOGMCP_ALLOWED_CLIENT_IDS` when mode=google → error
- Add test for missing `GOGMCP_JWT_SECRET` when mode=jwt → error
- Add test for default mode (no `GOGMCP_AUTH_MODE` set) defaults to `"google"`
- Add test for invalid mode → error
- Update existing tests that set `GOGMCP_JWT_SECRET` to also set `GOGMCP_AUTH_MODE=jwt`

### Step 2: Update Claims (`internal/auth/types.go`)

**Current:**
```go
type Claims struct {
    Email             string `json:"email"`
    AuthType          string `json:"auth_type"`
    GoogleAccessToken string `json:"google_access_token,omitempty"`
    jwt.RegisteredClaims
}

type AuthContext struct {
    Claims      *Claims
    GoogleToken string
}
```

**Target:**
```go
// GoogleClaims represents claims from a Google ID token.
type GoogleClaims struct {
    Email         string `json:"email"`
    EmailVerified bool   `json:"email_verified"`
    jwt.RegisteredClaims
    // aud is validated separately against the allowlist
    // iss is validated to be "https://accounts.google.com"
}

// Claims represents the legacy HMAC JWT claims (jwt mode only).
type Claims struct {
    Email             string `json:"email"`
    AuthType          string `json:"auth_type"`
    GoogleAccessToken string `json:"google_access_token,omitempty"`
    jwt.RegisteredClaims
}

// AuthContext holds request-scoped authentication data.
type AuthContext struct {
    Email       string // User email (from either auth mode)
    GoogleToken string // Google access token for API calls
}
```

**Key changes:**
- `AuthContext` is simplified — it just holds the email and Google token regardless of auth mode
- `GoogleClaims` is the new struct for Google OIDC mode
- `Claims` is kept for legacy `jwt` mode backward compatibility
- `AuthContext.Claims` pointer is removed — consumers only need the email

**Impact on consumers of `AuthContext`:**
- `internal/mcp/tools.go` — `withGoogleTokenContext` accesses `authCtx.GoogleToken` — **unchanged**
- Tests that access `authCtx.Claims.Email` → change to `authCtx.Email`

### Step 3: Update Errors (`internal/auth/errors.go`)

**Add:**
```go
// ErrInvalidAudience indicates the token's aud claim is not in the allowed list
ErrInvalidAudience = errors.New("token audience not allowed")

// ErrEmailNotVerified indicates the Google account email is not verified
ErrEmailNotVerified = errors.New("email not verified")

// ErrJWKSFetchFailed indicates failure to fetch Google's public keys
ErrJWKSFetchFailed = errors.New("failed to fetch Google JWKS keys")
```

**Keep all existing errors** — they're still used in both modes.

### Step 4: Create Google ID Token Validator (`internal/auth/google.go`)

This is a **new file**. It validates Google ID tokens using Google's public JWKS keys.

**Dependencies to add to `go.mod`:**
- None new — `golang-jwt/jwt/v5` already supports RS256
- For JWKS fetching, use `crypto/rsa` + `encoding/json` + `net/http` (all stdlib)
- OR use `github.com/MicahParks/keyfunc/v2` for automatic JWKS caching (recommended, less code)

**Option A: Manual JWKS (no new deps, more code):**

```go
// @awa-component: AUTH-GoogleIDValidator
package auth

import (
    "crypto/rsa"
    "encoding/base64"
    "encoding/json"
    "fmt"
    "math/big"
    "net/http"
    "sync"
    "time"

    "github.com/golang-jwt/jwt/v5"
)

const (
    googleJWKSURL    = "https://www.googleapis.com/oauth2/v3/certs"
    googleIssuer     = "https://accounts.google.com"
    googleIssuerAlt  = "accounts.google.com"
    jwksCacheTTL     = 6 * time.Hour
)

// GoogleIDValidator validates Google ID tokens using JWKS public keys.
type GoogleIDValidator struct {
    allowedAudiences map[string]bool
    keys             map[string]*rsa.PublicKey
    keysMu           sync.RWMutex
    lastFetch        time.Time
    httpClient       *http.Client
}

// NewGoogleIDValidator creates a validator with allowed audience (client ID) list.
func NewGoogleIDValidator(allowedClientIDs []string) *GoogleIDValidator {
    auds := make(map[string]bool, len(allowedClientIDs))
    for _, id := range allowedClientIDs {
        auds[id] = true
    }
    return &GoogleIDValidator{
        allowedAudiences: auds,
        keys:             make(map[string]*rsa.PublicKey),
        httpClient:       &http.Client{Timeout: 10 * time.Second},
    }
}
```

**JWKS fetching and caching:**

```go
// jwksResponse represents Google's JWKS endpoint response.
type jwksResponse struct {
    Keys []jwksKey `json:"keys"`
}

type jwksKey struct {
    Kid string `json:"kid"` // Key ID
    Kty string `json:"kty"` // Key type (RSA)
    Alg string `json:"alg"` // Algorithm (RS256)
    N   string `json:"n"`   // RSA modulus (base64url)
    E   string `json:"e"`   // RSA exponent (base64url)
}

// fetchKeys fetches Google's JWKS keys and caches them.
func (v *GoogleIDValidator) fetchKeys() error {
    resp, err := v.httpClient.Get(googleJWKSURL)
    if err != nil {
        return fmt.Errorf("%w: %v", ErrJWKSFetchFailed, err)
    }
    defer resp.Body.Close()

    if resp.StatusCode != http.StatusOK {
        return fmt.Errorf("%w: status %d", ErrJWKSFetchFailed, resp.StatusCode)
    }

    var jwks jwksResponse
    if err := json.NewDecoder(resp.Body).Decode(&jwks); err != nil {
        return fmt.Errorf("%w: %v", ErrJWKSFetchFailed, err)
    }

    keys := make(map[string]*rsa.PublicKey)
    for _, k := range jwks.Keys {
        if k.Kty != "RSA" {
            continue
        }
        pubKey, err := parseRSAPublicKey(k.N, k.E)
        if err != nil {
            continue // Skip malformed keys
        }
        keys[k.Kid] = pubKey
    }

    v.keysMu.Lock()
    v.keys = keys
    v.lastFetch = time.Now()
    v.keysMu.Unlock()

    return nil
}

// getKey returns the RSA public key for the given key ID.
// Fetches/refreshes JWKS if cache is stale or key is unknown.
func (v *GoogleIDValidator) getKey(kid string) (*rsa.PublicKey, error) {
    v.keysMu.RLock()
    key, ok := v.keys[kid]
    stale := time.Since(v.lastFetch) > jwksCacheTTL
    v.keysMu.RUnlock()

    if ok && !stale {
        return key, nil
    }

    // Fetch fresh keys
    if err := v.fetchKeys(); err != nil {
        // If we have a cached key, use it even if stale
        if ok {
            return key, nil
        }
        return nil, err
    }

    v.keysMu.RLock()
    key, ok = v.keys[kid]
    v.keysMu.RUnlock()

    if !ok {
        return nil, ErrInvalidToken
    }
    return key, nil
}

// parseRSAPublicKey converts base64url-encoded N and E to an RSA public key.
func parseRSAPublicKey(nStr, eStr string) (*rsa.PublicKey, error) {
    nBytes, err := base64.RawURLEncoding.DecodeString(nStr)
    if err != nil {
        return nil, err
    }
    eBytes, err := base64.RawURLEncoding.DecodeString(eStr)
    if err != nil {
        return nil, err
    }

    n := new(big.Int).SetBytes(nBytes)
    e := new(big.Int).SetBytes(eBytes)

    return &rsa.PublicKey{
        N: n,
        E: int(e.Int64()),
    }, nil
}
```

**Validation method:**

```go
// Validate validates a Google ID token and returns the claims.
func (v *GoogleIDValidator) Validate(tokenString string) (*GoogleClaims, error) {
    // Parse token, looking up the signing key by kid
    token, err := jwt.ParseWithClaims(tokenString, &GoogleClaims{}, func(token *jwt.Token) (interface{}, error) {
        // Verify signing method is RSA
        if _, ok := token.Method.(*jwt.SigningMethodRSA); !ok {
            return nil, fmt.Errorf("unexpected signing method: %v", token.Header["alg"])
        }

        // Get key ID from token header
        kid, ok := token.Header["kid"].(string)
        if !ok || kid == "" {
            return nil, ErrInvalidToken
        }

        // Fetch the corresponding public key
        return v.getKey(kid)
    })

    if err != nil {
        if errors.Is(err, jwt.ErrTokenExpired) {
            return nil, ErrTokenExpired
        }
        return nil, ErrInvalidToken
    }

    claims, ok := token.Claims.(*GoogleClaims)
    if !ok || !token.Valid {
        return nil, ErrInvalidToken
    }

    // Validate issuer
    issuer, _ := claims.GetIssuer()
    if issuer != googleIssuer && issuer != googleIssuerAlt {
        return nil, ErrInvalidToken
    }

    // Validate audience against allowlist
    audiences, _ := claims.GetAudience()
    allowed := false
    for _, aud := range audiences {
        if v.allowedAudiences[aud] {
            allowed = true
            break
        }
    }
    if !allowed {
        return nil, ErrInvalidAudience
    }

    // Validate email
    if claims.Email == "" {
        return nil, ErrMissingClaims
    }

    // Validate email is verified
    if !claims.EmailVerified {
        return nil, ErrEmailNotVerified
    }

    return claims, nil
}
```

**Option B: Using `keyfunc` library (less code, one new dep):**

If you prefer fewer lines, add `github.com/MicahParks/keyfunc/v2` and the JWKS fetching + caching + RSA parsing is handled automatically. The `Validate` method becomes ~20 lines. Up to you — Option A has zero new dependencies.

### Step 5: Update Context Function (`internal/auth/context.go`)

**Current `ContextFunc`** does:
1. Extract JWT from Authorization header
2. Validate JWT (HMAC)
3. If `auth_type == "oauth"`, require Google access token
4. If `auth_type == "service_account"`, skip Google token requirement
5. Store `AuthContext` in context

**New `ContextFunc`** needs to support both modes.

**Target implementation:**

```go
// TokenValidator is the interface both validators implement.
type TokenValidator interface {
    // ValidateAndExtract validates the token and returns (email, error).
    ValidateAndExtract(tokenString string) (string, error)
}

// ContextFunc returns an SSE context function for the google auth mode.
func GoogleContextFunc(validator *GoogleIDValidator) func(ctx context.Context, r *http.Request) context.Context {
    return func(ctx context.Context, r *http.Request) context.Context {
        // Extract token from Authorization header
        authHeader := r.Header.Get("Authorization")
        if authHeader == "" {
            return context.WithValue(ctx, authContextKey, ErrMissingToken)
        }

        tokenString := strings.TrimPrefix(authHeader, "Bearer ")
        if tokenString == authHeader {
            tokenString = strings.TrimPrefix(authHeader, "bearer ")
        }

        // Validate Google ID token
        claims, err := validator.Validate(tokenString)
        if err != nil {
            return context.WithValue(ctx, authContextKey, err)
        }

        // Always require Google access token (google mode = always oauth)
        googleToken := r.Header.Get("X-Google-Access-Token")
        if googleToken == "" {
            return context.WithValue(ctx, authContextKey, ErrMissingGoogleToken)
        }

        authCtx := &AuthContext{
            Email:       claims.Email,
            GoogleToken: googleToken,
        }

        return context.WithValue(ctx, authContextKey, authCtx)
    }
}
```

**Keep the existing `ContextFunc`** renamed to `JWTContextFunc` for legacy mode:

```go
// JWTContextFunc returns an SSE context function for the legacy jwt auth mode.
func JWTContextFunc(validator *JWTValidator) func(ctx context.Context, r *http.Request) context.Context {
    // ... (existing implementation, updated to use new AuthContext shape)
}
```

**Update `AuthFromContext`:**

The function stays the same — it still pulls `AuthContext` or an error from context. But `AuthContext` now has `Email` string instead of `Claims *Claims`.

### Step 6: Update `JWTValidator` (`internal/auth/jwt.go`)

Keep it as-is for backward compatibility (jwt mode). Only change: update it to populate the new `AuthContext` shape if needed by the context func.

The existing `JWTValidator.Validate()` signature returns `(*Claims, error)` — keep this. The `JWTContextFunc` (renamed from `ContextFunc`) will map `Claims.Email` → `AuthContext.Email`.

### Step 7: Update Server (`internal/mcp/server.go`)

**Current:**
```go
func NewServer(cfg *config.Config, validator *auth.JWTValidator) (*Server, error) {
    // ...
    contextFunc := auth.ContextFunc(validator)
    // ...
}
```

**Target:**
```go
func NewServer(cfg *config.Config) (*Server, error) {
    // ...

    // Create auth context function based on mode
    var contextFunc func(ctx context.Context, r *http.Request) context.Context
    switch cfg.AuthMode {
    case "google":
        validator := auth.NewGoogleIDValidator(cfg.AllowedClientIDs)
        contextFunc = auth.GoogleContextFunc(validator)
    case "jwt":
        validator := auth.NewJWTValidator([]byte(cfg.JWTSecret))
        contextFunc = auth.JWTContextFunc(validator)
    }

    // ... rest unchanged (SSE server creation, guard handler, etc.)
}
```

**Signature change:** `NewServer` no longer takes a `*auth.JWTValidator` parameter — it creates the right validator internally based on config.

**Impact on `main.go`** (wherever gogmcp's main entrypoint is — likely in `cmd/gogmcp/main.go` or similar):
- Remove the `auth.NewJWTValidator([]byte(cfg.JWTSecret))` call
- Change `mcp.NewServer(cfg, validator)` → `mcp.NewServer(cfg)`

**Auth status helper (`authStatus` func):**

Add `ErrInvalidAudience` and `ErrEmailNotVerified`:
```go
func authStatus(err error) int {
    switch {
    case errors.Is(err, auth.ErrMissingClaims),
         errors.Is(err, auth.ErrMissingGoogleToken),
         errors.Is(err, auth.ErrEmailNotVerified):
        return http.StatusBadRequest
    case errors.Is(err, auth.ErrInvalidAudience):
        return http.StatusForbidden
    case errors.Is(err, auth.ErrServiceAccountNotConfigured):
        return http.StatusInternalServerError
    default:
        return http.StatusUnauthorized
    }
}
```

### Step 8: Update `tools.go` (`internal/mcp/tools.go`)

**Current `withGoogleTokenContext`:**
```go
func withGoogleTokenContext(next server.ToolHandlerFunc, quotaProject string) server.ToolHandlerFunc {
    return func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
        authCtx, err := auth.AuthFromContext(ctx)
        if err != nil {
            return mcp.NewToolResultError(err.Error()), nil
        }
        // ...
        if authCtx != nil && authCtx.GoogleToken != "" {
            ctx = google.ContextWithToken(ctx, google.TokenFromString(authCtx.GoogleToken))
        }
        return next(ctx, req)
    }
}
```

This already works with the new `AuthContext` — it only accesses `authCtx.GoogleToken` which is unchanged. **No changes needed.**

### Step 9: Update `cmd/testjwt/` → `cmd/testtoken/`

The `testjwt` tool mints HMAC JWTs for testing. With the new design, you can't mint Google ID tokens (Google does that). Options:

**Option A: Keep for jwt mode testing.**
Rename to make it clear it's for legacy mode:
- No code changes, just document that it's for `GOGMCP_AUTH_MODE=jwt` only

**Option B: Add a mock Google ID token generator for integration tests.**
Create a test helper that:
1. Generates an RSA key pair
2. Serves a fake JWKS endpoint
3. Mints ID tokens signed with the RSA key
4. Configures the validator to use the fake JWKS URL

This is useful for integration tests but not needed for initial implementation — tests can use Option A or test against real Google tokens.

**Recommendation: Option A** for now. Keep `testjwt` unchanged, document it's for legacy mode. Tests for google mode should mock at the validator level.

### Step 10: Update Tests

#### `internal/auth/jwt_test.go`
- **No changes** — these test the legacy `JWTValidator` which still exists for jwt mode

#### `internal/auth/context_test.go`
- **Rename relevant functions** for clarity:
  - `TestContextFunc_*` → `TestJWTContextFunc_*`
- **Add new tests** for `GoogleContextFunc`:
  - `TestGoogleContextFunc_MissingAuthorizationHeader` → `ErrMissingToken`
  - `TestGoogleContextFunc_InvalidToken` → `ErrInvalidToken`
  - `TestGoogleContextFunc_ExpiredToken` → `ErrTokenExpired`
  - `TestGoogleContextFunc_WrongAudience` → `ErrInvalidAudience`
  - `TestGoogleContextFunc_MissingGoogleAccessToken` → `ErrMissingGoogleToken`
  - `TestGoogleContextFunc_ValidToken` → success, check `authCtx.Email` and `authCtx.GoogleToken`
  - `TestGoogleContextFunc_UnverifiedEmail` → `ErrEmailNotVerified`
- For Google context tests, you need **test RSA keys** to sign fake ID tokens:

```go
// Test helper: create a test Google ID token signed with a test RSA key
func createTestGoogleIDToken(t *testing.T, key *rsa.PrivateKey, kid string, claims *GoogleClaims) string {
    token := jwt.NewWithClaims(jwt.SigningMethodRS256, claims)
    token.Header["kid"] = kid
    tokenString, err := token.SignedString(key)
    if err != nil {
        t.Fatalf("failed to sign test token: %v", err)
    }
    return tokenString
}
```

And a test validator that has the test public key pre-loaded (bypassing JWKS fetch):

```go
// NewTestGoogleIDValidator creates a validator with pre-loaded keys for testing.
func NewTestGoogleIDValidator(allowedClientIDs []string, keys map[string]*rsa.PublicKey) *GoogleIDValidator {
    auds := make(map[string]bool, len(allowedClientIDs))
    for _, id := range allowedClientIDs {
        auds[id] = true
    }
    return &GoogleIDValidator{
        allowedAudiences: auds,
        keys:             keys,
        lastFetch:        time.Now(), // Prevent JWKS fetch
        httpClient:       &http.Client{Timeout: 10 * time.Second},
    }
}
```

#### New file: `internal/auth/google_test.go`

Test the `GoogleIDValidator.Validate()` method:
- Valid token with correct audience → success
- Valid token with wrong audience → `ErrInvalidAudience`
- Expired token → `ErrTokenExpired`
- Invalid signature → `ErrInvalidToken`
- Missing email → `ErrMissingClaims`
- Unverified email → `ErrEmailNotVerified`
- Wrong issuer → `ErrInvalidToken`
- HMAC-signed token (not RSA) → `ErrInvalidToken` (signing method check)
- Missing kid header → `ErrInvalidToken`

#### `internal/config/config_test.go`
- Update as described in Step 1

#### `internal/mcp/server_test.go`
- Update `NewServer` calls: remove `validator` parameter, pass config with `AuthMode: "jwt"` + `JWTSecret` for existing tests
- Add test for `AuthMode: "google"` with `AllowedClientIDs`

#### `internal/mcp/tools_test.go`
- Tests access `authCtx.Claims.Email` → change to `authCtx.Email`
- Otherwise unchanged

### Step 11: Update `doc/SECURITY.md`

Update the Security Architecture document to reflect the new design:
- Add Google OIDC section
- Document the two auth modes
- Update the threat model
- Update configuration examples

### Step 12: Update `README.md`

Update the Configuration table:

| Variable | Required | Description |
|----------|----------|-------------|
| `GOGMCP_PORT` | No | Server port (default: `8443`) |
| `GOGMCP_TLS_CERT` | Yes | Path to TLS certificate |
| `GOGMCP_TLS_KEY` | Yes | Path to TLS private key |
| `GOGMCP_AUTH_MODE` | No | `google` (default) or `jwt` |
| `GOGMCP_ALLOWED_CLIENT_IDS` | If google mode | Comma-separated OAuth client IDs for `aud` validation |
| `GOGMCP_JWT_SECRET` | If jwt mode | Legacy HMAC shared secret |
| `GOGMCP_SERVICE_ACCOUNT_FILE` | No | Service account (jwt mode only) |
| `GOGMCP_QUOTA_PROJECT` | No | Google Cloud quota/billing project |

### Step 13: Update awa Specs (if maintaining)

Update `.awa/specs/REQ-AUTH-authentication.md`:
- Add new requirements for Google OIDC mode (AUTH-5: Google OIDC Validation)
- Mark AUTH-1 (JWT Validation) as applicable to legacy jwt mode
- Add AUTH-5_AC-1 through AUTH-5_AC-N for the new mode

Update `.awa/specs/DESIGN-MCP-server.md`:
- Document the `GoogleIDValidator` component
- Update the `AUTH-JWTValidator` section to note it's legacy

## Implementation Order

Execute in this order, running `make test` after each step:

1. **Step 3** — Add new errors (non-breaking)
2. **Step 2** — Update types (add `GoogleClaims`, update `AuthContext`)
3. **Step 4** — Create `google.go` validator (new file, no existing code touched)
4. **Step 10 (google_test.go only)** — Add validator unit tests
5. **Step 1** — Update config (add `AuthMode`, `AllowedClientIDs`)
6. **Step 5** — Update context.go (rename + add `GoogleContextFunc`)
7. **Step 10 (context_test.go)** — Update context tests
8. **Step 6** — Verify jwt.go needs no changes
9. **Step 7** — Update server.go
10. **Step 10 (server_test.go, tools_test.go)** — Update remaining tests
11. **Step 8** — Verify tools.go needs no changes
12. **Step 9** — Document testjwt as legacy-mode tool
13. **Steps 11-13** — Update docs and specs

## Testing Strategy

### Unit Tests (mock keys, no network)

All Google OIDC tests use pre-generated RSA test keys. No network calls. The `GoogleIDValidator` should have a test constructor that accepts pre-loaded keys.

### Integration Test (optional, real Google tokens)

For manual verification:
1. Get a real Google ID token (e.g., via `gcloud auth print-identity-token`)
2. Run gogmcp with `GOGMCP_AUTH_MODE=google` and the corresponding client ID
3. Connect with the real ID token

### Backward Compatibility Test

1. Run gogmcp with `GOGMCP_AUTH_MODE=jwt` and `GOGMCP_JWT_SECRET`
2. Use `cmd/testjwt` to generate a token
3. Connect and verify existing behavior works unchanged

## Rollback Plan

If any issue arises, set `GOGMCP_AUTH_MODE=jwt` and `GOGMCP_JWT_SECRET` to revert to the old behavior. No code changes needed — both modes are always compiled in.

## Dependencies Summary

| Dependency | Current | Change |
|------------|---------|--------|
| `github.com/golang-jwt/jwt/v5` | Already in go.mod | No change (RS256 support built-in) |
| `golang.org/x/oauth2` | Already in go.mod | No change |
| `google.golang.org/api` | Already in go.mod | No change |
| `crypto/rsa`, `encoding/json`, etc. | stdlib | No change |
| New dependencies | — | **None required** (Option A: manual JWKS) |
