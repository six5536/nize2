// @zen-component: PLAN-032-UseServerForm

/**
 * Hook managing MCP server form state, validation, and config building.
 *
 * Used by the unified ServerForm component for both create and edit modes.
 */

"use client";

import { useState, useMemo } from "react";
import type { AuthType, OAuthConfig, ServerConfig, ServerFormValues, TransportType } from "./types";

interface EnvPair {
  key: string;
  value: string;
}

interface UseServerFormReturn {
  // Common fields
  name: string;
  setName: (v: string) => void;
  domain: string;
  setDomain: (v: string) => void;
  description: string;
  setDescription: (v: string) => void;
  visibility: "hidden" | "visible";
  setVisibility: (v: "hidden" | "visible") => void;
  transport: TransportType;
  setTransport: (v: TransportType) => void;

  // HTTP fields
  url: string;
  setUrl: (v: string) => void;
  authType: AuthType;
  setAuthType: (v: AuthType) => void;
  apiKey: string;
  setApiKey: (v: string) => void;

  // OAuth fields
  clientId: string;
  setClientId: (v: string) => void;
  clientSecret: string;
  setClientSecret: (v: string) => void;
  oauthScopes: string;
  setOauthScopes: (v: string) => void;
  authorizationUrl: string;
  setAuthorizationUrl: (v: string) => void;
  tokenUrl: string;
  setTokenUrl: (v: string) => void;

  // Stdio fields
  command: string;
  setCommand: (v: string) => void;
  args: string;
  setArgs: (v: string) => void;
  envPairs: EnvPair[];
  setEnvPairs: (v: EnvPair[]) => void;

  // Computed
  isValid: boolean;
  buildConfig: () => ServerConfig;
  buildOAuthConfig: () => OAuthConfig | undefined;
  hasOAuthConfigChanged: boolean;
}

// @zen-impl: PLAN-032 Step 6
export function useServerForm(initialValues?: ServerFormValues, options?: { mode?: "create" | "edit" }): UseServerFormReturn {
  const mode = options?.mode || "create";
  const cfg = initialValues?.config || {};
  const initialTransport = (cfg.transport as TransportType) || initialValues?.transport || "http";
  const initialAuthType = (cfg.authType as AuthType) || initialValues?.authType || "none";

  // Common fields
  const [name, setName] = useState(initialValues?.name || "");
  const [domain, setDomain] = useState(initialValues?.domain || "");
  const [description, setDescription] = useState(initialValues?.description || "");
  const [visibility, setVisibility] = useState<"hidden" | "visible">(initialValues?.visibility === "hidden" ? "hidden" : "visible");
  const [transport, setTransport] = useState<TransportType>(initialTransport);

  // HTTP fields
  const [url, setUrl] = useState((cfg.url as string) || "");
  const [authType, setAuthType] = useState<AuthType>(initialAuthType);
  const [apiKey, setApiKey] = useState("");

  // OAuth fields
  const [clientId, setClientId] = useState(initialValues?.oauthConfig?.clientId || "");
  const [clientSecret, setClientSecret] = useState("");
  const [oauthScopes, setOauthScopes] = useState(initialValues?.oauthConfig?.scopes?.join(" ") || "openid email profile");
  const [authorizationUrl, setAuthorizationUrl] = useState(initialValues?.oauthConfig?.authorizationUrl || "https://accounts.google.com/o/oauth2/v2/auth");
  const [tokenUrl, setTokenUrl] = useState(initialValues?.oauthConfig?.tokenUrl || "https://oauth2.googleapis.com/token");

  // Stdio fields
  const [command, setCommand] = useState((cfg.command as string) || "");
  const [args, setArgs] = useState(Array.isArray(cfg.args) ? (cfg.args as string[]).join(" ") : "");
  const [envPairs, setEnvPairs] = useState<EnvPair[]>(() => {
    const env = cfg.env as Record<string, string> | undefined;
    if (env && typeof env === "object") {
      return Object.entries(env).map(([key, value]) => ({ key, value }));
    }
    return [];
  });

  const isValid = useMemo(() => {
    if (!name || !domain) return false;
    if (transport === "stdio") return command.length > 0;
    // HTTP — in edit mode, URL is optional (keep existing)
    if (mode === "create" && !url) return false;
    if (authType === "api-key" && !apiKey) return false;
    if (authType === "oauth" && !clientId) return false;
    return true;
  }, [name, domain, transport, command, url, authType, apiKey, clientId, mode]);

  const buildConfig = (): ServerConfig => {
    if (transport === "stdio") {
      const env: Record<string, string> = {};
      for (const pair of envPairs) {
        if (pair.key) env[pair.key] = pair.value;
      }
      return {
        transport: "stdio",
        command,
        args: args ? args.split(/\s+/) : undefined,
        env: Object.keys(env).length > 0 ? env : undefined,
      };
    }
    return {
      transport: "http",
      url,
      authType,
      apiKey: authType === "api-key" ? apiKey : undefined,
    };
  };

  const buildOAuthConfig = (): OAuthConfig | undefined => {
    if (authType !== "oauth") return undefined;
    return {
      clientId,
      authorizationUrl,
      tokenUrl,
      scopes: oauthScopes.split(/[\s,]+/).filter(Boolean),
    };
  };

  const hasOAuthConfigChanged = useMemo(() => {
    if (authType !== "oauth" || !initialValues?.oauthConfig) return false;
    const orig = initialValues.oauthConfig;
    return clientId !== (orig.clientId || "") || oauthScopes !== (orig.scopes?.join(" ") || "openid email profile") || authorizationUrl !== (orig.authorizationUrl || "https://accounts.google.com/o/oauth2/v2/auth") || tokenUrl !== (orig.tokenUrl || "https://oauth2.googleapis.com/token") || clientSecret.length > 0;
  }, [authType, clientId, oauthScopes, authorizationUrl, tokenUrl, clientSecret, initialValues?.oauthConfig]);

  return {
    name,
    setName,
    domain,
    setDomain,
    description,
    setDescription,
    visibility,
    setVisibility,
    transport,
    setTransport,
    url,
    setUrl,
    authType,
    setAuthType,
    apiKey,
    setApiKey,
    clientId,
    setClientId,
    clientSecret,
    setClientSecret,
    oauthScopes,
    setOauthScopes,
    authorizationUrl,
    setAuthorizationUrl,
    tokenUrl,
    setTokenUrl,
    command,
    setCommand,
    args,
    setArgs,
    envPairs,
    setEnvPairs,
    isValid,
    buildConfig,
    buildOAuthConfig,
    hasOAuthConfigChanged,
  };
}
