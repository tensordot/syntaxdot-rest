use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;

use anyhow::Result;
use async_std::task::spawn;
use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use udgraph::graph::{Comment, Sentence};

enum MetadataState {
    Sentences,
    Annotate(Pin<Box<dyn Future<Output = Result<Vec<Sentence>, anyhow::Error>> + Send + Sync>>),
}

pub struct Metadata<S> {
    pipeline_name: String,
    sentences: Pin<Box<S>>,
    state: MetadataState,
}

impl<S> Metadata<S>
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    pub fn new(pipeline_name: String, sentences: S) -> Self {
        Metadata {
            pipeline_name,
            sentences: Box::pin(sentences),
            state: MetadataState::Sentences,
        }
    }
}

impl<S> Stream for Metadata<S>
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    type Item = Result<Vec<Sentence>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Metadata {
            pipeline_name,
            sentences,
            state,
        } = &mut *self;

        loop {
            match state {
                MetadataState::Sentences => match ready!(sentences.as_mut().poll_next(cx)) {
                    None => return Poll::Ready(None),
                    Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                    Some(Ok(sentences)) => {
                        let mut sentences_with_metadata = sentences.clone();

                        let pipeline_name = pipeline_name.clone();

                        let future = spawn(async move {
                            for sentence in &mut sentences_with_metadata {
                                sentence.comments_mut().push(Comment::AttrVal {
                                    attr: "pipeline".to_string(),
                                    val: pipeline_name.to_owned(),
                                });
                            }

                            Ok(sentences_with_metadata)
                        });
                        *state = MetadataState::Annotate(Box::pin(future));
                    }
                },
                MetadataState::Annotate(future) => match ready!(future.as_mut().poll(cx)) {
                    Err(err) => {
                        return Poll::Ready(Some(Err(Error::new(ErrorKind::InvalidData, err))))
                    }
                    Ok(sentences) => {
                        *state = MetadataState::Sentences;
                        return Poll::Ready(Some(Ok(sentences)));
                    }
                },
            }
        }
    }
}

pub trait ToMetadata<S> {
    fn metadata(self, pipline_name: impl ToString) -> Metadata<S>;
}

impl<S> ToMetadata<S> for S
where
    S: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    fn metadata(self, pipline_name: impl ToString) -> Metadata<S> {
        Metadata::new(pipline_name.to_string(), self)
    }
}
