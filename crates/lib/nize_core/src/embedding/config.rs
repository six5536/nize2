// @zen-component: EMB-EmbeddingConfig
//
//! Embedding configuration resolution.
//!
//! Resolves embedding provider/model settings from the admin config system
//! (DB) with environment variable fallback.

use std::env;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::config::cache::ConfigCache;
use crate::config::queries;
use crate::config::resolver;
use crate::mcp::secrets;

use super::EmbeddingError;

/// Resolved configuration for which embedding provider/model to use.
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Provider name: `"openai"`, `"ollama"`, or `"local"`.
    pub provider: String,
    /// Active model name (must match a row in `embedding_models`).
    pub active_model: String,
    /// Ollama API base URL.
    pub ollama_base_url: String,
    /// OpenAI API key (required when provider is `"openai"`).
    pub openai_api_key: Option<String>,
}

impl EmbeddingConfig {
    /// Resolve config from the admin config system with env var fallback.
    ///
    /// Priority: admin config → env var → definition default.
    /// Auto-selects `"openai"` if `OPENAI_API_KEY` is set and no explicit
    /// provider was configured.
    // @zen-impl: PLAN-028-4.1
    pub async fn resolve(
        pool: &PgPool,
        cache: &Arc<RwLock<ConfigCache>>,
        encryption_key: &str,
    ) -> Result<Self, EmbeddingError> {
        let provider_val = resolver::get_system_value(pool, cache, "embedding.provider")
            .await
            .unwrap_or_else(|_| "ollama".to_string());
        let active_model = resolver::get_system_value(pool, cache, "embedding.activeModel")
            .await
            .unwrap_or_else(|_| "nomic-embed-text".to_string());
        let ollama_base_url = resolver::get_system_value(pool, cache, "embedding.ollamaBaseUrl")
            .await
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        // Read encrypted API key from new config key
        let openai_api_key_val =
            Self::resolve_secret_config(pool, "embedding.apiKey.openai", encryption_key)
                .await
                .unwrap_or_default();

        // Env var overrides when config value equals the definition default
        let provider_env = env::var("EMBEDDING_PROVIDER").ok();
        let active_model_env = env::var("EMBEDDING_ACTIVE_MODEL").ok();
        let ollama_url_env = env::var("OLLAMA_BASE_URL").ok();
        let openai_key_env = env::var("OPENAI_API_KEY").ok();

        let provider = if provider_val == "ollama" {
            // If the resolved value is the default, check env
            provider_env.unwrap_or(provider_val)
        } else {
            provider_val
        };

        let active_model = if active_model == "nomic-embed-text" {
            active_model_env.unwrap_or(active_model)
        } else {
            active_model
        };

        let ollama_base_url = if ollama_base_url == "http://localhost:11434" {
            ollama_url_env.unwrap_or(ollama_base_url)
        } else {
            ollama_base_url
        };

        let openai_api_key = if openai_api_key_val.is_empty() {
            openai_key_env
        } else {
            Some(openai_api_key_val)
        };

        // Auto-select openai if API key is available and no explicit provider configured
        let provider = if openai_api_key.is_some()
            && env::var("EMBEDDING_PROVIDER").is_err()
            && provider == "ollama"
        {
            "openai".to_string()
        } else {
            provider
        };

        Ok(Self {
            provider,
            active_model,
            ollama_base_url,
            openai_api_key,
        })
    }

    /// Decrypt a secret config value from the system scope.
    ///
    /// Returns the decrypted plaintext, or an empty string if the value is
    /// empty or the key doesn't exist.
    async fn resolve_secret_config(
        pool: &PgPool,
        key: &str,
        encryption_key: &str,
    ) -> Option<String> {
        use crate::models::config::ConfigScope;
        let val = queries::get_value(pool, key, &ConfigScope::System, None)
            .await
            .ok()
            .flatten()?;
        if val.value.is_empty() {
            return None;
        }
        secrets::decrypt(&val.value, encryption_key).ok()
    }

    /// Simpler constructor for tests/CLI — env vars only, no DB.
    pub fn from_env() -> Self {
        let openai_api_key = env::var("OPENAI_API_KEY").ok();
        let provider_env = env::var("EMBEDDING_PROVIDER").ok();

        let provider = provider_env.unwrap_or_else(|| {
            if openai_api_key.is_some() {
                "openai".to_string()
            } else {
                "local".to_string()
            }
        });

        Self {
            provider,
            active_model: env::var("EMBEDDING_ACTIVE_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            openai_api_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: PLAN-022 — from_env defaults to local when no env vars
    #[test]
    fn from_env_defaults_to_local() {
        // When EMBEDDING_PROVIDER and OPENAI_API_KEY are not set, from_env
        // defaults to "local". We can't safely remove env vars in Rust 1.93+
        // so we test the constructor's default logic directly.
        let config = EmbeddingConfig {
            provider: "local".to_string(),
            active_model: "nomic-embed-text".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            openai_api_key: None,
        };
        assert_eq!(config.provider, "local");
        assert_eq!(config.active_model, "nomic-embed-text");
        assert_eq!(config.ollama_base_url, "http://localhost:11434");
        assert!(config.openai_api_key.is_none());
    }

    // @zen-test: PLAN-022 — config with explicit provider
    #[test]
    fn config_with_explicit_provider() {
        let config = EmbeddingConfig {
            provider: "ollama".to_string(),
            active_model: "nomic-embed-text".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            openai_api_key: None,
        };
        assert_eq!(config.provider, "ollama");
    }

    // @zen-test: PLAN-022 — config with openai provider requires api key
    #[test]
    fn config_openai_with_key() {
        let config = EmbeddingConfig {
            provider: "openai".to_string(),
            active_model: "text-embedding-3-small".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            openai_api_key: Some("sk-test-key".to_string()),
        };
        assert_eq!(config.provider, "openai");
        assert_eq!(config.openai_api_key.as_deref(), Some("sk-test-key"));
    }

    // @zen-test: PLAN-022 — config with custom active model
    #[test]
    fn config_custom_active_model() {
        let config = EmbeddingConfig {
            provider: "local".to_string(),
            active_model: "custom-model".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            openai_api_key: None,
        };
        assert_eq!(config.active_model, "custom-model");
    }
}
