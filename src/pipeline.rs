use std::sync::Arc;

use alpino_tokenizer::Tokenizer;
use futures::io::Error;
use futures::stream::Stream;
use udgraph::graph::Sentence;

use crate::annotator::Annotator;
use crate::async_syntaxdot::{
    Normalization, ToAnnotations, ToMetadata, ToSentences, ToUnicodeCleanup,
};
use crate::async_util::ToTryChunks;

/// An annotation pipeline.
#[derive(Clone)]
pub struct Pipeline {
    annotator: Arc<Annotator>,
    tokenizer: Arc<dyn Tokenizer + Send + Sync>,
    batch_size: usize,
    description: String,
    name: String,
    read_ahead: usize,
}

impl Pipeline {
    /// Construct a new pipeline.
    pub fn new(
        description: impl ToString,
        name: impl ToString,
        annotator: Annotator,
        tokenizer: Arc<dyn Tokenizer + Send + Sync>,
        batch_size: usize,
        read_ahead: usize,
    ) -> Self {
        Self {
            annotator: Arc::new(annotator),
            tokenizer,
            batch_size,
            description: description.to_string(),
            name: name.to_string(),
            read_ahead,
        }
    }

    /// Annotate a text stream.
    pub fn annotations<S>(&self, text_stream: S) -> impl Stream<Item = Result<Vec<Sentence>, Error>>
    where
        S: Stream<Item = Result<String, Error>>,
    {
        self.sentences(text_stream)
            .try_chunks(self.batch_size * self.read_ahead)
            .annotations(self.annotator.clone(), self.batch_size)
            .metadata(self.name())
    }

    /// Pipeline description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Pipeline name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tokenize sentences and apply unicode cleanup.
    pub fn sentences<S>(&self, text_stream: S) -> impl Stream<Item = Result<Sentence, Error>>
    where
        S: Stream<Item = Result<String, Error>>,
    {
        text_stream
            .sentences(self.tokenizer.clone())
            .unicode_cleanup(Normalization::Nfc)
    }
}
