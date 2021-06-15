use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Arc;

use alpino_tokenizer::{AlpinoTokenizer, Tokenizer};
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use tch::Device;

use crate::annotator::Annotator;
use crate::pipeline::Pipeline;
use crate::tokenizer::WhitespaceTokenizer;

/// SyntaxDot REST server configuration
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Tokenizer + annotation pipelines
    annotators: IndexMap<String, AnnotatorConfig>,

    /// Pipelines
    pipelines: IndexMap<String, PipelineConfig>,

    /// Tokenizers
    tokenizers: IndexMap<String, TokenizerConfig>,
}

impl Config {
    /// Read a SyntaxDot REST server configuration file.
    pub fn read<P, R>(config_path: P, mut read: R) -> Result<Self>
    where
        P: AsRef<Path>,
        R: Read,
    {
        let mut yaml = String::new();
        read.read_to_string(&mut yaml)?;

        let mut config: Config = serde_yaml::from_str(&yaml)?;

        for tokenizer_config in config.tokenizers.values_mut() {
            if let TokenizerConfig::AlpinoTokenizer(ref mut protobuf) = tokenizer_config {
                *protobuf = canonicalize_path(config_path.as_ref(), &protobuf)?;
            }
        }

        for annotator_config in config.annotators.values_mut() {
            annotator_config.syntaxdot_config =
                canonicalize_path(config_path.as_ref(), &annotator_config.syntaxdot_config)?;
        }

        Ok(config)
    }

    /// Load tokenizers and annotators and construct pipelines out of them.
    pub fn load(&self) -> Result<IndexMap<String, Pipeline>> {
        let mut tokenizers = IndexMap::new();
        for (name, tokenizer_config) in &self.tokenizers {
            let tokenizer = tokenizer_config.load()?;
            tokenizers.insert(name.to_string(), tokenizer);
        }

        let mut annotators = IndexMap::new();
        for (name, annotator_config) in &self.annotators {
            let annotator = annotator_config.load()?;
            annotators.insert(name.to_string(), Arc::new(annotator));
        }

        let mut pipelines = IndexMap::new();
        for (name, pipeline_config) in &self.pipelines {
            let pipeline = pipeline_config.new_pipeline(name, &annotators, &tokenizers)?;
            pipelines.insert(name.to_string(), pipeline);
        }

        Ok(pipelines)
    }
}

/// Annotator configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct AnnotatorConfig {
    /// Maximum sentence length in pieces.
    max_len: Option<usize>,

    /// SyntaxDot model configuration.
    syntaxdot_config: String,
}

impl AnnotatorConfig {
    /// Load an annotator.
    fn load(&self) -> Result<Annotator> {
        Annotator::load(Device::Cpu, &self.syntaxdot_config, self.max_len)
    }
}

/// Pipeline configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct PipelineConfig {
    /// Name of the annotator to use.
    annotator: String,

    /// Batch size.
    batch_size: usize,

    /// Pipeline description.
    description: String,

    /// Number of batches to read ahead.
    read_ahead: usize,

    /// Name of the tokenizer to use.
    tokenizer: String,
}

impl PipelineConfig {
    fn new_pipeline(
        &self,
        name: &str,
        annotators: &IndexMap<String, Arc<Annotator>>,
        tokenizers: &IndexMap<String, Arc<dyn Tokenizer + Send + Sync>>,
    ) -> Result<Pipeline> {
        let annotator = annotators
            .get(&self.annotator)
            .ok_or_else(|| anyhow!("Unknown annotator `{}`", self.annotator))?;

        let tokenizer = tokenizers
            .get(&self.tokenizer)
            .ok_or_else(|| anyhow!("Unknown tokenizer `{}`", self.tokenizer))?;

        Ok(Pipeline::new(
            &self.description,
            name,
            annotator.clone(),
            tokenizer.clone(),
            self.batch_size,
            self.read_ahead,
        ))
    }
}

/// Configuration for a tokenizer.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenizerConfig {
    /// Alpino tokenizer.
    AlpinoTokenizer(
        /// Tokenizer protobuf file.
        String,
    ),

    /// Whitespace tokenizer.
    ///
    /// Splits sentences on `\n` or `\r\n`, split tokens on any ASCII whitespace.
    WhitespaceTokenizer,
}

impl TokenizerConfig {
    /// Load a tokenizer.
    pub fn load(&self) -> Result<Arc<dyn Tokenizer + Send + Sync>> {
        match self {
            TokenizerConfig::AlpinoTokenizer(protobuf) => {
                let read = BufReader::new(File::open(protobuf)?);
                Ok(Arc::new(AlpinoTokenizer::from_buf_read(read)?))
            }
            TokenizerConfig::WhitespaceTokenizer => Ok(Arc::new(WhitespaceTokenizer)),
        }
    }
}

/// Canonicalize a (relative) filename.
///
/// The configuration file can contain file names relative to the configuration
/// file directory. However, the program can be run in a different directory
/// than the configuration. This function gives the absolute path of a file
/// name that is relative to a configuration file directory.
fn canonicalize_path(config_path: &Path, filename: &str) -> Result<String> {
    if filename.is_empty() {
        return Ok(filename.to_owned());
    }

    let path = Path::new(&filename);

    // Don't touch absolute paths.
    if path.is_absolute() {
        return Ok(filename.to_owned());
    }

    let abs_config_path = config_path.canonicalize()?;
    Ok(abs_config_path
        .parent()
        .ok_or_else(|| {
            anyhow!(
                "Cannot get parent path of the configuration file: {}",
                abs_config_path.to_string_lossy()
            )
        })?
        .join(path)
        .canonicalize()?
        .to_str()
        .ok_or_else(|| {
            anyhow!(
                "Cannot cannot convert parent path to string: {}",
                abs_config_path.to_string_lossy()
            )
        })?
        .to_owned())
}
