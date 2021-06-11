use std::collections::VecDeque;
use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;

use alpino_tokenizer::Tokenizer;
use async_std::task::spawn;
use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use udgraph::graph::Sentence;
use udgraph::token::Token;

type TokenizedSentences = Vec<Vec<String>>;

enum SentencesState {
    Lines,
    Tokenize(Pin<Box<dyn Future<Output = Option<TokenizedSentences>> + Send + Sync>>),
    Sentences(VecDeque<Sentence>),
}

/// Stream that tokenizes sentences.
/// Stream that tokenizes sentences.
pub struct Sentences<L> {
    lines: Pin<Box<L>>,
    state: SentencesState,
    tokenizer: Arc<dyn Tokenizer + Send + Sync>,
}

impl<L> Sentences<L>
where
    L: Stream<Item = Result<String, Error>>,
{
    pub fn new(tokenizer: Arc<dyn Tokenizer + Send + Sync>, lines: L) -> Self {
        Sentences {
            lines: Box::pin(lines),
            state: SentencesState::Lines,
            tokenizer,
        }
    }
}

impl<L> Stream for Sentences<L>
where
    L: Stream<Item = Result<String, Error>>,
{
    type Item = Result<Sentence, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Sentences {
            lines,
            state,
            tokenizer,
        } = &mut *self;

        loop {
            match state {
                SentencesState::Lines => match ready!(lines.as_mut().poll_next(cx)) {
                    None => return Poll::Ready(None),
                    Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                    Some(Ok(line)) => {
                        // Do not process empty lines.
                        if line.trim().is_empty() {
                            continue;
                        }

                        let tokenizer = tokenizer.clone();
                        let future = spawn(async move { tokenizer.tokenize(&line) });
                        *state = SentencesState::Tokenize(Box::pin(future));
                    }
                },
                SentencesState::Tokenize(future) => match ready!(future.as_mut().poll(cx)) {
                    None => {
                        return Poll::Ready(Some(Err(Error::new(
                            ErrorKind::InvalidData,
                            "Cannot tokenize data".to_string(),
                        ))))
                    }
                    Some(tokens) => {
                        let sentences = tokens
                            .into_iter()
                            .map(|s| s.into_iter().map(Token::new).collect::<Sentence>())
                            .collect();
                        *state = SentencesState::Sentences(sentences);
                    }
                },
                SentencesState::Sentences(sentences) => {
                    if sentences.is_empty() {
                        *state = SentencesState::Lines;
                        continue;
                    }

                    return Poll::Ready(Some(Ok(sentences
                        .pop_front()
                        .expect("Attempted to pop from empty buffer?"))));
                }
            }
        }
    }
}

pub trait ToSentences<L> {
    fn sentences(self, tokenizer: Arc<dyn Tokenizer + Send + Sync>) -> Sentences<L>;
}

impl<L> ToSentences<L> for L
where
    L: Stream<Item = Result<String, Error>>,
{
    fn sentences(self, tokenizer: Arc<dyn Tokenizer + Send + Sync>) -> Sentences<L> {
        Sentences::new(tokenizer, self)
    }
}
