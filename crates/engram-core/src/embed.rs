//! Local text embeddings via candle (pure Rust, no ONNX).
//!
//! Runs `sentence-transformers/all-MiniLM-L6-v2` (384-dim) on CPU. Model files
//! are downloaded once from the HuggingFace hub on first use and cached. We
//! pick candle over fastembed/ort because ort ships no prebuilt binaries for
//! the GNU toolchain.

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

pub const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";
pub const DIM: usize = 384;
const MAX_TOKENS: usize = 384;

/// A loaded embedding model. Loading downloads ~90MB on first run.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl Embedder {
    /// Download (if needed) and load the model on CPU.
    pub fn load() -> Result<Self> {
        let device = Device::Cpu;
        let api = hf_hub::api::sync::Api::new().context("init hf-hub api")?;
        let repo = api.model(MODEL_ID.to_string());

        let config_path = repo.get("config.json").context("download config.json")?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .context("download tokenizer.json")?;

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

        // Prefer safetensors; fall back to the PyTorch checkpoint.
        let vb = if let Ok(safetensors) = repo.get("model.safetensors") {
            unsafe {
                VarBuilder::from_mmaped_safetensors(&[safetensors], DType::F32, &device)?
            }
        } else {
            let pth = repo
                .get("pytorch_model.bin")
                .context("download model weights")?;
            VarBuilder::from_pth(pth, DType::F32, &device)?
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
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
