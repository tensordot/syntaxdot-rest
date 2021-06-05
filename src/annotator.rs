use std::fs::File;
use std::io::BufReader;
use std::ops::Deref;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::{ParallelIterator, ParallelSliceMut};
use syntaxdot::config::{BiaffineParserConfig, Config, PretrainConfig, TomlRead};
use syntaxdot::encoders::Encoders;
use syntaxdot::model::bert::BertModel;
use syntaxdot::tagger::Tagger;
use syntaxdot_encoders::dependency::ImmutableDependencyEncoder;
use syntaxdot_tch_ext::RootExt;
use syntaxdot_tokenizers::{SentenceWithPieces, Tokenize};
use tch::nn::VarStore;
use tch::Device;
use udgraph::graph::Sentence;

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
    max_len: Option<usize>,
    tagger: TaggerWrap,
    tokenizer: Box<dyn Tokenize>,
}

impl Annotator {
    pub fn load<P>(device: Device, config_path: P, max_len: Option<usize>) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let r = BufReader::new(File::open(&config_path)?);
        let mut config = Config::from_toml_read(r)?;
        config.relativize_paths(config_path)?;

        let biaffine_decoder = config
            .biaffine
            .as_ref()
            .map(|config| load_biaffine_decoder(config))
            .transpose()?;

        let encoders = load_encoders(&config)?;
        let tokenizer = load_tokenizer(&config)?;
        let pretrain_config = load_pretrain_config(&config)?;

        let mut vs = VarStore::new(device);

        let model = BertModel::new(
            vs.root_ext(|_| 0),
            &pretrain_config,
            config.biaffine.as_ref(),
            biaffine_decoder
                .as_ref()
                .map(ImmutableDependencyEncoder::n_relations)
                .unwrap_or(0),
            &encoders,
            config.model.pooler,
            0.0,
            config.model.position_embeddings.clone(),
        )
        .context("Cannot construct model")?;

        vs.load(&config.model.parameters)
            .context("Cannot load model parameters")?;

        vs.freeze();

        let tagger = Tagger::new(device, model, biaffine_decoder, encoders);

        Ok(Annotator {
            max_len,
            tagger: TaggerWrap(tagger),
            tokenizer,
        })
    }

    pub fn annotate_sentences(
        &self,
        sentences: &[Sentence],
        batch_size: usize,
    ) -> Result<Vec<SentenceWithPieces>> where {
        let mut sentences_with_pieces = sentences
            .iter()
            .map(|s| self.tokenizer.tokenize(s.clone()))
            .filter(|s| {
                self.max_len
                    .map(|max_len| s.pieces.len() <= max_len)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();

        // Sort sentences by length.
        let mut sent_refs: Vec<_> = sentences_with_pieces.iter_mut().collect();
        sent_refs.sort_unstable_by_key(|s| s.pieces.len());

        // Convince the type system that we are not borrowing Annotator, which is
        // not Sync.
        let tagger = &self.tagger;

        // Split in batches, tag, and merge results.
        sent_refs
            .par_chunks_mut(batch_size)
            .try_for_each(|batch| tagger.tag_sentences(batch))?;

        Ok(sentences_with_pieces)
    }
}

pub fn load_pretrain_config(config: &Config) -> Result<PretrainConfig> {
    config
        .model
        .pretrain_config()
        .context("Cannot load pretraining model configuration")
}

fn load_biaffine_decoder(config: &BiaffineParserConfig) -> Result<ImmutableDependencyEncoder> {
    let f = File::open(&config.labels).context(format!(
        "Cannot open dependency label file: {}",
        config.labels
    ))?;

    let encoder: ImmutableDependencyEncoder = serde_yaml::from_reader(&f).context(format!(
        "Cannot deserialize dependency labels from: {}",
        config.labels
    ))?;

    log::info!("Loaded biaffine encoder: {} labels", encoder.n_relations());

    Ok(encoder)
}

fn load_encoders(config: &Config) -> Result<Encoders> {
    let f = File::open(&config.labeler.labels)
        .context(format!("Cannot open label file: {}", config.labeler.labels))?;
    let encoders: Encoders = serde_yaml::from_reader(&f).context(format!(
        "Cannot deserialize labels from: {}",
        config.labeler.labels
    ))?;

    for encoder in &*encoders {
        log::info!(
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
