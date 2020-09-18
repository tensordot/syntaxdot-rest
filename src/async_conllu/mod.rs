use std::pin::Pin;

use conllu::{
    graph::Sentence,
    io::{WriteSentence, Writer},
};
use futures::io::{AsyncRead, Error, ErrorKind};
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};

pub struct SentenceStreamReader<A> {
    annotations: Pin<Box<A>>,
    first_output: bool,
    parse_buf: Vec<u8>,
}

impl<A> SentenceStreamReader<A> {
    pub fn new(annotations: A) -> Self {
        SentenceStreamReader {
            first_output: true,
            annotations: Box::pin(annotations),
            parse_buf: Vec::new(),
        }
    }
}

impl<A> AsyncRead for SentenceStreamReader<A>
where
    A: Stream<Item = Result<Vec<Sentence>, Error>>,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let SentenceStreamReader {
            annotations,
            first_output,
            parse_buf,
        } = &mut *self;

        if parse_buf.is_empty() {
            let sentences = match ready!(annotations.as_mut().poll_next(cx)) {
                None => return Poll::Ready(Ok(0)),
                Some(Err(err)) => return Poll::Ready(Err(err)),
                Some(Ok(sentences)) => sentences,
            };

            let mut write_buf = if *first_output {
                *first_output = false;
                Vec::new()
            } else {
                vec![b'\n']
            };

            let mut writer = Writer::new(&mut write_buf);
            for sentence in sentences {
                if let Err(err) = writer.write_sentence(&sentence) {
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
