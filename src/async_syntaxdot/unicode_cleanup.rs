use std::pin::Pin;

use conllu::graph::{Node, Sentence};
use futures::io::Error;
use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};

use super::unicode::{simplify_unicode, Normalization};

fn cleanup_sentence_unicode(sentence: &mut Sentence, normalization: Normalization) {
    for token in sentence.iter_mut().filter_map(Node::token_mut) {
        let form = token.form();
        let clean_form = simplify_unicode(form, normalization);

        if form != clean_form {
            let form = form.to_string();
            token.misc_mut().insert("orth".to_string(), Some(form));
            token.set_form(clean_form);
        }
    }
}

pub struct UnicodeCleanup<L> {
    sentences: Pin<Box<L>>,
    normalization: Normalization,
}

impl<L> UnicodeCleanup<L>
where
    L: Stream<Item = Result<Sentence, Error>>,
{
    pub fn new(normalization: Normalization, sentences: L) -> Self {
        Self {
            sentences: Box::pin(sentences),
            normalization,
        }
    }
}

impl<L> Stream for UnicodeCleanup<L>
where
    L: Stream<Item = Result<Sentence, Error>>,
{
    type Item = Result<Sentence, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let Self {
            sentences,
            normalization,
        } = &mut *self;

        match ready!(sentences.as_mut().poll_next(cx)) {
            None => Poll::Ready(None),
            Some(Err(err)) => Poll::Ready(Some(Err(err))),
            Some(Ok(mut sentence)) => {
                cleanup_sentence_unicode(&mut sentence, *normalization);
                Poll::Ready(Some(Ok(sentence)))
            }
        }
    }
}

pub trait ToUnicodeCleanup<L> {
    fn unicode_cleanup(self, normalization: Normalization) -> UnicodeCleanup<L>;
}

impl<L> ToUnicodeCleanup<L> for L
where
    L: Stream<Item = Result<Sentence, Error>>,
{
    fn unicode_cleanup(self, normalization: Normalization) -> UnicodeCleanup<L> {
        UnicodeCleanup::new(normalization, self)
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use conllu::graph::Sentence;
    use conllu::token::{Token, TokenBuilder};
    use futures::executor::block_on_stream;
    use futures::stream::{self, StreamExt};

    use super::{Normalization, ToUnicodeCleanup};

    #[test]
    fn unicode_cleanup_works() {
        let sentence: Sentence = vec![Token::new("«"), Token::new("test"), Token::new("»")]
            .into_iter()
            .collect();
        let chunks = block_on_stream(
            stream::iter(vec![sentence])
                .map(Ok)
                .unicode_cleanup(Normalization::NFC),
        )
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

        let check_sentence = vec![
            TokenBuilder::new("\"")
                .misc(iter::once(("orth".to_string(), Some("«".to_string()))).collect())
                .into(),
            Token::new("test"),
            TokenBuilder::new("\"")
                .misc(iter::once(("orth".to_string(), Some("»".to_string()))).collect())
                .into(),
        ]
        .into_iter()
        .collect();

        assert_eq!(chunks, vec![check_sentence]);
    }
}
