use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write as _};
use std::sync::{Mutex, OnceLock};

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// Opt-in capture sink. When the `CAPTURE_PATH` env var is set, every request
/// body the handler parses is appended (raw, one per line) to that file so we
/// can replay the real load-test query distribution through the offline oracle
/// harness. When unset this is a cheap `&None` and adds no behavior to the
/// serving path.
static CAPTURE: OnceLock<Option<Mutex<BufWriter<File>>>> = OnceLock::new();

fn capture_sink() -> &'static Option<Mutex<BufWriter<File>>> {
    CAPTURE.get_or_init(|| match std::env::var("CAPTURE_PATH") {
        Ok(path) if !path.is_empty() => OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
            .map(|f| Mutex::new(BufWriter::new(f))),
        _ => None,
    })
}

/// Tee a raw request body to the capture sink when `CAPTURE_PATH` is set.
/// No-op (and no allocation) otherwise.
pub fn capture(bytes: &[u8]) {
    if let Some(sink) = capture_sink()
        && let Ok(mut w) = sink.lock()
    {
        // Bodies are compact single-line JSON, so newline framing is safe.
        let _ = w.write_all(bytes);
        let _ = w.write_all(b"\n");
        let _ = w.flush();
    }
}

pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match sonic_rs::to_vec(&self.0) {
            Ok(bytes) => ([(header::CONTENT_TYPE, "application/json")], bytes).into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}
