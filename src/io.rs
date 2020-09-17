use std::pin::Pin;
use std::sync::Arc;

use conllu::{
    graph::Sentence,
    io::{WriteSentence, Writer},
    token::Token,
};
use futures::io::{AsyncBufRead, AsyncRead, Error, ErrorKind, Lines};
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};

use crate::annotator::Annotator;

pub struct AnnotatorReader<R> {
    annotator: Arc<Annotator>,
    parse_buf: Vec<u8>,
    lines: Pin<Box<Lines<R>>>,
}

impl<R> AnnotatorReader<R> {
    pub fn new(annotator: Arc<Annotator>, lines: Lines<R>) -> Self {
        AnnotatorReader {
            annotator,
            parse_buf: Vec::new(),
            lines: Box::pin(lines),
        }
    }
}

impl<R> AsyncRead for AnnotatorReader<R>
where
    R: AsyncBufRead,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let AnnotatorReader {
            parse_buf,
            lines,
            annotator,
        } = &mut *self;

        if parse_buf.is_empty() {
            let line = match ready!(lines.as_mut().poll_next(cx)) {
                None => return Poll::Ready(Ok(0)),
                Some(Err(err)) => return Poll::Ready(Err(err)),
                Some(Ok(line)) => line,
            };

            let tokenized = match alpino_tokenizer::tokenize(&line) {
                Err(err) => return Poll::Ready(Err(Error::new(ErrorKind::InvalidData, err))),
                Ok(tokenized) => tokenized,
            };

            let mut sentences = tokenized
                .into_iter()
                .map(|s| s.into_iter().map(|t| Token::new(t)).collect::<Sentence>())
                .collect::<Vec<_>>();

            let sentences = match annotator.annotate_sentences(&mut sentences) {
                Err(err) => return Poll::Ready(Err(Error::new(ErrorKind::InvalidData, err))),
                Ok(sentences) => sentences,
            };

            let mut write_buf = Vec::new();

            let mut writer = Writer::new(&mut write_buf);
            for sentence in sentences {
                if let Err(err) = writer.write_sentence(&sentence.sentence) {
                    return Poll::Ready(Err(Error::new(ErrorKind::InvalidData, err)));
                }
            }

            std::mem::swap(&mut write_buf, parse_buf);
        }

        let bytes_to_copy = std::cmp::min(buf.len(), parse_buf.len());
        buf[..bytes_to_copy].copy_from_slice(&parse_buf[..bytes_to_copy]);
        parse_buf.drain(..bytes_to_copy);

        Poll::Ready(Ok(bytes_to_copy))
    }
}
