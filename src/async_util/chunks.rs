use std::pin::Pin;

use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};

pub struct TryChunks<St, I> {
    inner: Pin<Box<St>>,
    buf: Vec<I>,
    chunk_len: usize,
}

impl<St, I> TryChunks<St, I> {
    fn new(stream: St, chunk_len: usize) -> Self {
        Self {
            inner: Box::pin(stream),
            buf: Vec::with_capacity(chunk_len),
            chunk_len,
        }
    }
}

impl<St, I> Stream for TryChunks<St, I>
where
    St: Stream<Item = Result<I, Error>>,
    I: Unpin,
{
    type Item = Result<Vec<I>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let TryChunks {
            inner,
            buf,
            chunk_len,
        } = &mut *self;

        loop {
            match ready!(inner.as_mut().poll_next(cx)) {
                Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                Some(Ok(item)) => {
                    buf.push(item);

                    // Return the buffer if it reached the maximum length.
                    if buf.len() == *chunk_len {
                        let mut fresh = Vec::with_capacity(*chunk_len);
                        std::mem::swap(buf, &mut fresh);
                        return Poll::Ready(Some(Ok(fresh)));
                    }
                }
                None => {
                    if !buf.is_empty() {
                        let mut fresh = Vec::with_capacity(*chunk_len);
                        std::mem::swap(buf, &mut fresh);
                        return Poll::Ready(Some(Ok(fresh)));
                    }

                    return Poll::Ready(None);
                }
            }
        }
    }
}

pub trait ToTryChunks<St, I> {
    fn try_chunks(self, chunk_len: usize) -> TryChunks<St, I>;
}

impl<St, I> ToTryChunks<St, I> for St
where
    St: Stream<Item = Result<I, Error>>,
{
    fn try_chunks(self, chunk_len: usize) -> TryChunks<St, I> {
        TryChunks::new(self, chunk_len)
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on_stream;
    use futures::stream::{self, StreamExt};

    use super::ToTryChunks;

    #[test]
    fn can_chunk() {
        let chunks = block_on_stream(stream::iter(1..6).map(Ok).try_chunks(3))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(chunks, vec![vec![1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn can_chunk_empty_stream() {
        let chunks: Vec<Vec<usize>> = block_on_stream(stream::empty().try_chunks(3))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(chunks, vec![] as Vec<Vec<usize>>);
    }

    #[test]
    fn can_chunk_multiple_of_chunk_size() {
        let chunks = block_on_stream(stream::iter(1..=6).map(Ok).try_chunks(3))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(chunks, vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }
}
