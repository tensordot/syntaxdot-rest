use std::fs::File;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::{App, Arg};
use futures::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use indexmap::IndexMap;
use serde::Serialize;
use tide::{Body, Error, Request, Response, Server, StatusCode};

mod async_conllu;
use async_conllu::SentenceStreamReader;

mod async_syntaxdot;

mod async_util;
use async_util::ToTryChunks;

mod annotator;

mod config;
pub use config::{Config, PipelineConfig};

mod pipeline;
use pipeline::Pipeline;

mod tokenizer;

mod util;
use util::ServeFile;

#[derive(Serialize)]
struct PipelineDescription {
    name: String,
    description: String,
}

fn pipeline_from_request(request: &Request<State>) -> Result<&Pipeline, Error> {
    let pipeline_name: String = request.param("pipeline")?;

    request
        .state()
        .pipelines
        .get(&pipeline_name)
        .ok_or_else(|| {
            Error::new(
                StatusCode::NotFound,
                anyhow!("Unknown pipeline: {}", pipeline_name),
            )
        })
}

async fn handle_annotations(mut request: Request<State>) -> tide::Result {
    let body = request.take_body();
    let pipeline = pipeline_from_request(&request)?;

    let annotator_reader =
        SentenceStreamReader::new(pipeline.annotations(body.into_reader().lines()));

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_reader(
            AsyncBufReader::new(annotator_reader),
            None,
        ))
        .build())
}

async fn handle_pipelines(request: Request<State>) -> tide::Result {
    let pipelines = request
        .state()
        .pipelines
        .iter()
        .map(|(k, v)| PipelineDescription {
            name: k.to_string(),
            description: v.description().to_string(),
        })
        .collect::<Vec<_>>();

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_json(&pipelines)?)
        .build())
}

async fn handle_tokens(mut request: Request<State>) -> tide::Result {
    let body = request.take_body();
    let pipeline = pipeline_from_request(&request)?;

    let tokens_reader = SentenceStreamReader::new(
        pipeline
            .sentences(body.into_reader().lines())
            .try_chunks(16),
    );

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_reader(AsyncBufReader::new(tokens_reader), None))
        .build())
}

#[derive(Clone)]
struct State {
    pipelines: IndexMap<String, Pipeline>,
    config: Config,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("SyntaxDot REST server")
        .arg(Arg::with_name("config").required(true).index(1))
        .arg(
            Arg::with_name("static")
                .long("static")
                .takes_value(true)
                .help("Static files to serve"),
        )
        .get_matches();

    let config_filename = matches.value_of("config").unwrap();
    let config = Config::read(config_filename, File::open(config_filename)?)?;

    let pipelines = config.load()?;

    tide::log::start();
    let mut app = Server::with_state(State { pipelines, config });

    if let Some(dir) = matches.value_of("static") {
        let mut index_path = PathBuf::from(dir);
        index_path.push("index.html");

        app.at("/").get(ServeFile::new(index_path)?);
        app.at("/").serve_dir(dir)?;
    }

    app.at("/annotations/:pipeline").post(handle_annotations);
    app.at("/pipelines").get(handle_pipelines);
    app.at("/tokens/:pipeline").post(handle_tokens);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}
