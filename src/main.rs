use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use clap::{App, Arg};
use futures::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use sticker2::config::{Config, TomlRead};
use tch::Device;
use tide::{Body, Request, Response, Server, StatusCode};

mod async_conllu;
use async_conllu::SentenceStreamReader;

mod async_sticker;
use async_sticker::{Normalization, ToAnnotations, ToSentences, ToUnicodeCleanup};

mod async_util;
use async_util::ToTryChunks;

mod annotator;
use annotator::Annotator;

async fn handle_annotations(mut request: Request<State>) -> tide::Result {
    let annotator = request.state().annotator.clone();
    let annotator_reader = SentenceStreamReader::new(
        request
            .take_body()
            .into_reader()
            .lines()
            .sentences()
            .unicode_cleanup(Normalization::NFC)
            .try_chunks(16)
            .annotations(annotator),
    );

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_reader(
            AsyncBufReader::new(annotator_reader),
            None,
        ))
        .build())
}

async fn handle_tokens(mut request: Request<State>) -> tide::Result {
    let tokens_reader = SentenceStreamReader::new(
        request
            .take_body()
            .into_reader()
            .lines()
            .sentences()
            .unicode_cleanup(Normalization::NFC)
            .try_chunks(16),
    );

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_reader(AsyncBufReader::new(tokens_reader), None))
        .build())
}

#[derive(Clone)]
struct State {
    annotator: Arc<Annotator>,
}

fn load_model_config(filename: &str) -> anyhow::Result<Config> {
    let r = BufReader::new(File::open(filename)?);
    Config::from_toml_read(r)
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("sticker2 REST server")
        .arg(Arg::with_name("model").required(true).index(1))
        .get_matches();

    let annotator = annotator::Annotator::load(Device::Cpu, matches.value_of("model").unwrap())?;

    let mut config = load_model_config(matches.value_of("model").unwrap())?;
    config.relativize_paths(matches.value_of("model").unwrap())?;

    tide::log::start();
    let mut app = Server::with_state(State {
        annotator: Arc::new(annotator),
    });
    app.at("/").get(|_| async { Ok("Hello, world!") });
    app.at("/annotations").post(handle_annotations);
    app.at("/tokens").post(handle_tokens);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}
