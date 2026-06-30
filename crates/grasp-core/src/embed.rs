//! Local text embeddings via candle (pure Rust, no ONNX).
//!
//! Runs `sentence-transformers/all-MiniLM-L6-v2` (384-dim) on CPU. Model files
//! are downloaded once from the HuggingFace hub on first use and cached. We
//! pick candle over fastembed/ort because ort ships no prebuilt binaries for
//! the GNU toolchain.

use std::path::PathBuf;

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

pub const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";
pub const DIM: usize = 384;
const MAX_TOKENS: usize = 384;

/// A bundled model directory shipped next to the binary, e.g.
/// `<exe>/models/all-MiniLM-L6-v2/{config.json,tokenizer.json,model.safetensors}`.
/// Lets a release run offline with no first-use download.
fn bundled_model_dir() -> Option<PathBuf> {
    // Explicit override wins (also handy for the Tauri resource dir).
    if let Ok(dir) = std::env::var("GRASP_MODEL_DIR") {
        let p = PathBuf::from(dir);
        if p.join("config.json").exists() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("models").join("all-MiniLM-L6-v2");
    if dir.join("config.json").exists() && dir.join("model.safetensors").exists() {
        Some(dir)
    } else {
        None
    }
}

/// Resolve the three model files: a bundled copy if present, else download from
/// the HuggingFace hub (cached) on first use.
fn resolve_model_files() -> Result<(PathBuf, PathBuf, PathBuf)> {
    if let Some(dir) = bundled_model_dir() {
        return Ok((
            dir.join("config.json"),
            dir.join("tokenizer.json"),
            dir.join("model.safetensors"),
        ));
    }
    let api = hf_hub::api::sync::Api::new().context("init hf-hub api")?;
    let repo = api.model(MODEL_ID.to_string());
    Ok((
        repo.get("config.json").context("download config.json")?,
        repo.get("tokenizer.json").context("download tokenizer.json")?,
        repo.get("model.safetensors").context("download model weights")?,
    ))
}

/// A loaded embedding model. Loading downloads ~90MB on first run.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    /// Load the model on CPU — from a bundled copy next to the binary if present,
    /// otherwise downloading (and caching) from the HuggingFace hub on first use.
    pub fn load() -> Result<Self> {
        let device = Device::Cpu;
        let (config_path, tokenizer_path, weights_path) = resolve_model_files()?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)
            .context("parse bert config")?;

        let mut tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| anyhow::anyhow!("{e}"))?;
        tokenizer
            .with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                ..Default::default()
            }))
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)?
        };

        let model = BertModel::load(vb, &config).context("load bert model")?;
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Embed a batch of texts into L2-normalized 384-dim vectors.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let mut id_rows = Vec::with_capacity(encodings.len());
        let mut mask_rows = Vec::with_capacity(encodings.len());
        for enc in &encodings {
            id_rows.push(Tensor::new(enc.get_ids(), &self.device)?);
            mask_rows.push(Tensor::new(enc.get_attention_mask(), &self.device)?);
        }
        let input_ids = Tensor::stack(&id_rows, 0)?;
        let attention_mask = Tensor::stack(&mask_rows, 0)?;
        let token_type_ids = input_ids.zeros_like()?;

        let hidden = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))?; // [b, n, h]

        // Attention-masked mean pooling.
        let mask_f = attention_mask.to_dtype(DType::F32)?; // [b, n]
        let mask_exp = mask_f.unsqueeze(2)?; // [b, n, 1]
        let summed = hidden.broadcast_mul(&mask_exp)?.sum(1)?; // [b, h]
        let counts = mask_f.sum(1)?.unsqueeze(1)?; // [b, 1]
        let mean = summed.broadcast_div(&counts)?; // [b, h]

        // L2 normalize so cosine similarity == dot product.
        let norm = mean.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean.broadcast_div(&norm)?;
        Ok(normalized.to_vec2::<f32>()?)
    }

    /// Convenience: embed a single string.
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self
            .embed(&[text.to_string()])?
            .into_iter()
            .next()
            .unwrap_or_default())
    }
}

/// Cosine similarity of two equal-length, L2-normalized vectors (== dot product).
///
/// Returns 0.0 when the dimensions differ rather than silently computing a
/// truncated dot product over the shorter length — which `zip` would otherwise
/// do. Mismatched dimensions are reachable: the `embeddings` table stores a
/// per-row `dim`, so a future model/`DIM` change can leave stale vectors of a
/// different length that must not corrupt ranking or semantic edges.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_of_mismatched_dimensions_is_zero() {
        // A stale embedding of a different length must not yield a bogus
        // (truncated) similarity that could outrank correctly-sized vectors.
        assert_eq!(cosine(&[1.0, 0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn cosine_of_identical_unit_vectors_is_one() {
        let v = [0.6_f32, 0.8];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    /// Sanity check that the candle MiniLM port produces *meaningful* vectors:
    /// a query must be closer to a related sentence than an unrelated one, and
    /// vectors must be the right shape and L2-normalized.
    ///
    /// Ignored by default because it downloads the ~90MB model; run with:
    ///   cargo test -p grasp-core embeddings_are_meaningful -- --ignored
    #[test]
    #[ignore]
    fn embeddings_are_meaningful() {
        let e = Embedder::load().expect("load model");

        let query = e.embed_one("which database does the project use").unwrap();
        let related = e
            .embed_one("we store every memory in a local SQLite database")
            .unwrap();
        let unrelated = e
            .embed_one("the orange cat slept in the afternoon sun")
            .unwrap();

        // Right shape.
        assert_eq!(query.len(), DIM);
        // L2-normalized (self cosine ~= 1).
        assert!((cosine(&query, &query) - 1.0).abs() < 1e-3);

        let sim_related = cosine(&query, &related);
        let sim_unrelated = cosine(&query, &unrelated);
        // The related sentence must be clearly closer than the unrelated one.
        assert!(
            sim_related > sim_unrelated + 0.1,
            "related {sim_related:.3} should beat unrelated {sim_unrelated:.3}"
        );
    }
}
