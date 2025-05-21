use hyper::header::{HeaderValue, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_ALLOW_METHODS};

async fn router(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/download") => {
            let chunk_size = 1024 * 1024;
            let chunks_count = 100;

            let stream = stream::unfold(0, move |count| async move {
                if count >= chunks_count {
                    None
                } else {
                    let chunk = vec![0u8; chunk_size];
                    sleep(Duration::from_millis(50)).await;
                    Some((Ok::<Vec<u8>, Infallible>(chunk), count + 1))
                }
            });

            let mut response = Response::new(Body::wrap_stream(stream));

            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, OPTIONS"),
            );

            Ok(response)
        }
        (&Method::POST, "/upload") => {
            let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            let size = whole_body.len();
            println!("Ricevuti {} byte", size);
            let mut response = Response::new(Body::from("OK"));

            // Header CORS anche per POST
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, OPTIONS"),
            );

            Ok(response)
        }
        (&Method::OPTIONS, _) => {
            // Rispondi alle richieste OPTIONS per CORS preflight
            let mut response = Response::new(Body::empty());
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST, OPTIONS"),
            );
            response.headers_mut().insert(
                hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("content-type"),
            );
            Ok(response)
        }
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}
