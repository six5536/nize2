// @awa-component: EMB-LocalProvider
//
//! Local deterministic embedding provider using FNV-1a hash.
//!
//! Produces repeatable embeddings with no external dependencies — useful for
//! testing and offline development. Ported from the reference project's
//! `localEmbed()` in `packages/ingestion/src/embedder.ts`.

use super::EmbeddingResult;

/// Generate a deterministic embedding for a single text using FNV-1a hashing.
///
/// The algorithm seeds an FNV-1a hash from the input text, then uses an
/// xorshift PRNG to fill the vector with values in `[-1, 1]`.
pub fn embed(text: &str, dimensions: i32) -> Vec<f32> {
    // FNV-1a hash of the input text
    let mut seed: u32 = 2_166_136_261;
    for byte in text.bytes() {
        seed ^= byte as u32;
        seed = seed.wrapping_mul(16_777_619);
    }

    // Xorshift PRNG to fill the vector
    let dims = dimensions as usize;
    let mut vector = Vec::with_capacity(dims);
    let mut x = seed;
    for _ in 0..dims {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let normalized = (x as f64) / (u32::MAX as f64);
        vector.push((normalized * 2.0 - 1.0) as f32);
    }

    vector
}

/// Embed a batch of texts using the local deterministic provider.
pub fn embed_batch(texts: &[String], dimensions: i32, model: &str) -> Vec<EmbeddingResult> {
    texts
        .iter()
        .map(|text| EmbeddingResult {
            text: text.clone(),
            embedding: embed(text, dimensions),
            model: model.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // @awa-test: PLAN-022 — local provider determinism
    #[test]
    fn embed_is_deterministic() {
        let a = embed("hello world", 768);
        let b = embed("hello world", 768);
        assert_eq!(a, b);
    }

    // @awa-test: PLAN-022 — local provider dimensions
    #[test]
    fn embed_correct_dimensions() {
        let v = embed("test", 1536);
        assert_eq!(v.len(), 1536);

        let v = embed("test", 768);
        assert_eq!(v.len(), 768);
    }

    // @awa-test: PLAN-022 — different inputs produce different embeddings
    #[test]
    fn different_texts_produce_different_embeddings() {
        let a = embed("hello", 768);
        let b = embed("world", 768);
        assert_ne!(a, b);
    }

    // @awa-test: PLAN-022 — embeddings are in [-1, 1]
    #[test]
    fn values_in_expected_range() {
        let v = embed("test embedding range", 768);
        for val in &v {
            assert!(
                *val >= -1.0 && *val <= 1.0,
                "value {val} out of [-1, 1] range"
            );
        }
    }

    // @awa-test: PLAN-022 — embed_batch produces correct count
    #[test]
    fn embed_batch_correct_count() {
        let texts = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let results = embed_batch(&texts, 768, "local-test");
        assert_eq!(results.len(), 3);
        for r in &results {
            assert_eq!(r.embedding.len(), 768);
            assert_eq!(r.model, "local-test");
        }
    }
}
