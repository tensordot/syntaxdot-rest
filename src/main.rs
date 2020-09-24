use std::collections::HashMap;
use std::fs::File;

use anyhow::anyhow;
use clap::{App, Arg};
use futures::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tide::{Body, Error, Request, Response, Server, StatusCode};

mod async_conllu;
use async_conllu::SentenceStreamReader;

mod async_sticker;

mod async_util;
use async_util::ToTryChunks;

mod annotator;

mod config;
pub use config::{Config, PipelineConfig};

mod pipeline;
use pipeline::Pipeline;

mod util;

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

async fn handle_index(_request: Request<State>) -> tide::Result {
    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_file("static/index.html").await?)
        .build())
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
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    Ok(Response::builder(StatusCode::Ok)
        .body(Body::from_string(pipelines.join("\n")))
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
    pipelines: HashMap<String, Pipeline>,
    config: Config,
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("sticker2 REST server")
        .arg(Arg::with_name("config").required(true).index(1))
        .get_matches();

    let config = Config::read(File::open(matches.value_of("config").unwrap())?)?;

    let pipelines = config.load()?;

    tide::log::start();
    let mut app = Server::with_state(State { pipelines, config });
    app.at("/").get(handle_index);
    app.at("/").serve_dir("static/")?;
    app.at("/annotations/:pipeline").post(handle_annotations);
    app.at("/pipelines").get(handle_pipelines);
    app.at("/tokens/:pipeline").post(handle_tokens);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
}
