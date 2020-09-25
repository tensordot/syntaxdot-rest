use std::io;
use std::os::raw::c_int;
use std::path::PathBuf;

use tide::{Body, Endpoint, Request, Response, StatusCode};

#[allow(dead_code)]
#[no_mangle]
extern "C" fn mkl_serv_intel_cpu_true() -> c_int {
    1
}

pub struct ServeFile(PathBuf);

impl ServeFile {
    pub fn new(path: impl Into<PathBuf>) -> io::Result<Self> {
        Ok(Self(path.into().canonicalize()?))
    }
}

#[async_trait::async_trait]
impl<State> Endpoint<State> for ServeFile
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, _req: Request<State>) -> tide::Result {
        Ok(Response::builder(StatusCode::Ok)
            .body(Body::from_file(&self.0).await?)
            .build())
    }
}
