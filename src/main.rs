use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use clap::{App, Arg};
use conllu::{
    graph::Sentence,
    io::{WriteSentence, Writer},
    token::Token,
};
use sticker2::config::{Config, TomlRead};
use tch::Device;
use tide::{Body, Request, Response, Server, StatusCode};

mod annotator;
use annotator::Annotator;

async fn handle_text(mut request: Request<State>) -> tide::Result {
    let text = request.body_string().await?;
    let tokenized = alpino_tokenizer::tokenize(&text)?;
    let sentences: Vec<_> = tokenized
        .into_iter()
        .map(|s| s.into_iter().map(Token::new).collect::<Sentence>())
        .collect();
    let sentences = request.state().annotator.annotate_sentences(&sentences)?;

    let mut data = Vec::new();
    let mut writer = Writer::new(&mut data);
    for sentence in sentences {
        writer.write_sentence(&sentence.sentence)?;
    }

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_bytes(data))
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
    app.at("/parse").post(handle_text);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}
