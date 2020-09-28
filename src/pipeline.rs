use std::sync::Arc;

use alpino_tokenizer::Tokenizer;
use conllu::graph::Sentence;
use futures::io::Error;
use futures::stream::Stream;

use crate::annotator::Annotator;
use crate::async_sticker::{Normalization, ToAnnotations, ToSentences, ToUnicodeCleanup};
use crate::async_util::ToTryChunks;

#[derive(Clone)]
pub struct Pipeline {
    annotator: Arc<Annotator>,
    tokenizer: Arc<dyn Tokenizer + Send + Sync>,
    batch_size: usize,
    description: String,
    read_ahead: usize,
}

impl Pipeline {
    pub fn new(
        description: String,
        annotator: Annotator,
        tokenizer: Arc<dyn Tokenizer + Send + Sync>,
        batch_size: usize,
        read_ahead: usize,
    ) -> Self {
        Self {
            annotator: Arc::new(annotator),
            tokenizer,
            batch_size,
            description,
            read_ahead,
        }
    }

    pub fn annotations<S>(&self, text_stream: S) -> impl Stream<Item = Result<Vec<Sentence>, Error>>
    where
        S: Stream<Item = Result<String, Error>>,
    {
        self.sentences(text_stream)
            .try_chunks(self.batch_size * self.read_ahead)
            .annotations(self.annotator.clone(), self.batch_size)
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn sentences<S>(&self, text_stream: S) -> impl Stream<Item = Result<Sentence, Error>>
    where
        S: Stream<Item = Result<String, Error>>,
    {
        text_stream
            .sentences(self.tokenizer.clone())
            .unicode_cleanup(Normalization::NFC)
    }
}
