use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::Arc;

use alpino_tokenizer::{AlpinoTokenizer, Tokenizer};
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use tch::Device;

use crate::annotator::Annotator;
use crate::pipeline::Pipeline;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    tokenizers: IndexMap<String, TokenizerConfig>,
    pipelines: IndexMap<String, PipelineConfig>,
}

impl Config {
    pub fn read<R>(mut read: R) -> Result<Self>
    where
        R: Read,
    {
        let mut toml = String::new();
        read.read_to_string(&mut toml)?;

        Ok(toml::from_str(&toml)?)
    }

    pub fn load(&self) -> Result<IndexMap<String, Pipeline>> {
        let mut tokenizers = IndexMap::new();
        for (name, tokenizer_config) in &self.tokenizers {
            let tokenizer = tokenizer_config.load()?;
            tokenizers.insert(name.to_string(), tokenizer);
        }

        let mut pipelines = IndexMap::new();

        for (language, pipeline_config) in &self.pipelines {
            let pipeline = pipeline_config.load(&tokenizers)?;
            pipelines.insert(language.to_string(), pipeline);
        }

        Ok(pipelines)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PipelineConfig {
    /// Batch size.
    batch_size: usize,

    /// Pipeline description.
    description: String,

    /// Number of batches to read ahead.
    read_ahead: usize,

    /// Sticker model configuration.
    sticker_config: String,

    /// Name of the tokenizer to use.
    tokenizer: String,
}

impl PipelineConfig {
    pub fn load(
        &self,
        tokenizers: &IndexMap<String, Arc<dyn Tokenizer + Send + Sync>>,
    ) -> Result<Pipeline> {
        let tokenizer = tokenizers
            .get(&self.tokenizer)
            .ok_or_else(|| anyhow!("Unknown tokenizer `{}`", self.tokenizer))?;
        let annotator = Annotator::load(Device::Cpu, &self.sticker_config)?;
        Ok(Pipeline::new(
            self.description.clone(),
            annotator,
            tokenizer.clone(),
            self.batch_size,
            self.read_ahead,
        ))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TokenizerConfig {
    /// Tokenizer protobuf file.
    protobuf: String,
}

impl TokenizerConfig {
    pub fn load(&self) -> Result<Arc<dyn Tokenizer + Send + Sync>> {
        let read = BufReader::new(File::open(&self.protobuf)?);
        Ok(Arc::new(AlpinoTokenizer::from_buf_read(read)?))
    }
}
