use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use async_std::task::spawn;
use conllu::graph::Sentence;
use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use syntaxdot_tokenizers::SentenceWithPieces;

use crate::annotator::Annotator;

enum AnnotationsState {
    Sentences,
    Annotate(
        Pin<Box<dyn Future<Output = Result<Vec<SentenceWithPieces>, anyhow::Error>> + Send + Sync>>,
    ),
}

pub struct Annotations<S> {
    annotator: Arc<Annotator>,
    batch_size: usize,
    sentences: Pin<Box<S>>,
    state: AnnotationsState,
}

impl<S> Annotations<S>
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    pub fn new(annotator: Arc<Annotator>, batch_size: usize, sentences: S) -> Self {
        Annotations {
            annotator,
            batch_size,
            sentences: Box::pin(sentences),
            state: AnnotationsState::Sentences,
        }
    }
}

impl<S> Stream for Annotations<S>
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    type Item = Result<Vec<Sentence>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Annotations {
            annotator,
            batch_size,
            sentences,
            state,
        } = &mut *self;

        // Ensure that we do not borrow batch_size in the closure.
        let batch_size = *batch_size;

        loop {
            match state {
                AnnotationsState::Sentences => match ready!(sentences.as_mut().poll_next(cx)) {
                    None => return Poll::Ready(None),
                    Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                    Some(Ok(sentences)) => {
                        let annotator = annotator.clone();
                        let future =
                            spawn(
                                async move { annotator.annotate_sentences(&sentences, batch_size) },
                            );
                        *state = AnnotationsState::Annotate(Box::pin(future));
                    }
                },
                AnnotationsState::Annotate(future) => match ready!(future.as_mut().poll(cx)) {
                    Err(err) => {
                        return Poll::Ready(Some(Err(Error::new(ErrorKind::InvalidData, err))))
                    }
                    Ok(sentences) => {
                        let sentences = sentences.into_iter().map(|s| s.sentence).collect();
                        *state = AnnotationsState::Sentences;
                        return Poll::Ready(Some(Ok(sentences)));
                    }
                },
            }
        }
    }
}

pub trait ToAnnotations<S> {
    fn annotations(self, annotator: Arc<Annotator>, batch_size: usize) -> Annotations<S>;
}

impl<S> ToAnnotations<S> for S
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    fn annotations(self, annotator: Arc<Annotator>, batch_size: usize) -> Annotations<S> {
        Annotations::new(annotator, batch_size, self)
    }
}
