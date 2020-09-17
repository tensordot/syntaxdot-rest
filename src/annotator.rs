use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::path::Path;

use anyhow::{Context, Result};
use conllu::graph::Sentence;
use sticker2::config::{Config, PretrainConfig, TomlRead};
use sticker2::encoders::Encoders;
use sticker2::input::{SentenceWithPieces, Tokenize};
use sticker2::model::bert::BertModel;
use sticker2::tagger::Tagger;
use tch::nn::VarStore;
use tch::Device;

/// A wrapper of `Tagger` that is `Send + Sync`.
///
/// Tensors are not thread-safe in the general case, but
/// multi-threaded use is safe if no (in-place) modifications are
/// made:
///
/// https://discuss.pytorch.org/t/is-evaluating-the-network-thread-safe/37802
struct TaggerWrap(Tagger);

unsafe impl Send for TaggerWrap {}

unsafe impl Sync for TaggerWrap {}

impl Deref for TaggerWrap {
    type Target = Tagger;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Annotator {
    tagger: TaggerWrap,
    tokenizer: Box<dyn Tokenize>,
}

impl Annotator {
    pub fn load<P>(device: Device, config_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let r = BufReader::new(File::open(&config_path)?);
        let mut config = Config::from_toml_read(r)?;
        config.relativize_paths(config_path)?;

        let encoders = load_encoders(&config)?;
        let tokenizer = load_tokenizer(&config)?;
        let pretrain_config = load_pretrain_config(&config)?;

        let mut vs = VarStore::new(device);

        let model = BertModel::new(
            vs.root(),
            &pretrain_config,
            &encoders,
            0.0,
            config.model.position_embeddings.clone(),
        )
        .context("Cannot construct model")?;

        vs.load(&config.model.parameters)
            .context("Cannot load model parameters")?;

        vs.freeze();

        let tagger = Tagger::new(device, model, encoders);

        Ok(Annotator {
            tagger: TaggerWrap(tagger),
            tokenizer,
        })
    }

    pub fn annotate_sentences(&self, sentences: &[Sentence]) -> Result<Vec<SentenceWithPieces>> where
    {
        let mut sentences_with_pieces = sentences
            .iter()
            .map(|s| self.tokenizer.tokenize(s.clone()))
            .collect::<Vec<_>>();

        self.tagger.tag_sentences(&mut sentences_with_pieces)?;

        Ok(sentences_with_pieces)
    }
}

pub fn load_pretrain_config(config: &Config) -> Result<PretrainConfig> {
    config
        .model
        .pretrain_config()
        .context("Cannot load pretraining model configuration")
}

fn load_encoders(config: &Config) -> Result<Encoders> {
    let f = File::open(&config.labeler.labels)
        .context(format!("Cannot open label file: {}", config.labeler.labels))?;
    let encoders: Encoders = serde_yaml::from_reader(&f).context(format!(
        "Cannot deserialize labels from: {}",
        config.labeler.labels
    ))?;

    for encoder in &*encoders {
        eprintln!(
            "Loaded labels for encoder '{}': {} labels",
            encoder.name(),
            encoder.encoder().len()
        );
    }

    Ok(encoders)
}

pub fn load_tokenizer(config: &Config) -> Result<Box<dyn Tokenize>> {
    config
        .tokenizer()
        .context("Cannot read tokenizer vocabulary")
}
