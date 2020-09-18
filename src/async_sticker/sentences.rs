use std::future::Future;
use std::io::ErrorKind;
use std::pin::Pin;

use async_std::task::spawn;
use conllu::graph::Sentence;
use conllu::token::Token;
use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};

type TokenizedSentences = Vec<Vec<String>>;

enum SentencesState {
    Lines,
    Tokenize(
        Pin<
            Box<
                dyn Future<Output = Result<TokenizedSentences, alpino_tokenizer::TokenizeError>>
                    + Send
                    + Sync,
            >,
        >,
    ),
}

pub struct Sentences<L> {
    lines: Pin<Box<L>>,
    state: SentencesState,
}

impl<L> Sentences<L>
where
    L: Stream<Item = Result<String, Error>>,
{
    pub fn new(lines: L) -> Self {
        Sentences {
            lines: Box::pin(lines),
            state: SentencesState::Lines,
        }
    }
}

impl<L> Stream for Sentences<L>
where
    L: Stream<Item = Result<String, Error>>,
{
    type Item = Result<Vec<Sentence>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Sentences { lines, state } = &mut *self;

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

                        let future = spawn(async move { alpino_tokenizer::tokenize(&line) });
                        *state = SentencesState::Tokenize(Box::pin(future));
                    }
                },
                SentencesState::Tokenize(future) => match ready!(future.as_mut().poll(cx)) {
                    Err(err) => {
                        return Poll::Ready(Some(Err(Error::new(ErrorKind::InvalidData, err))))
                    }
                    Ok(tokens) => {
                        let sentences = tokens
                            .into_iter()
                            .map(|s| s.into_iter().map(Token::new).collect::<Sentence>())
                            .collect();
                        *state = SentencesState::Lines;
                        return Poll::Ready(Some(Ok(sentences)));
                    }
                },
            }
        }
    }
}

pub trait ToSentences<L> {
    fn sentences(self) -> Sentences<L>;
}

impl<L> ToSentences<L> for L
where
    L: Stream<Item = Result<String, Error>>,
{
    fn sentences(self) -> Sentences<L> {
        Sentences::new(self)
    }
}
