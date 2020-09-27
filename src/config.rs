use std::collections::HashMap;
use std::io::Read;

use anyhow::Result;
use serde::Deserialize;
use tch::Device;

use crate::annotator::Annotator;
use crate::pipeline::Pipeline;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pipelines: HashMap<String, PipelineConfig>,
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

    pub fn load(&self) -> Result<HashMap<String, Pipeline>> {
        let mut pipelines = HashMap::new();

        for (language, pipeline_config) in &self.pipelines {
            let pipeline = pipeline_config.load()?;
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
}

impl PipelineConfig {
    pub fn load(&self) -> Result<Pipeline> {
        let annotator = Annotator::load(Device::Cpu, &self.sticker_config)?;
        Ok(Pipeline::new(
            self.description.clone(),
            annotator,
            self.batch_size,
            self.read_ahead,
        ))
    }
}
