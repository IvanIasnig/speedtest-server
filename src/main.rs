use hyper::{Body, Request, Response, Server, Method, StatusCode, HeaderMap};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use tokio::time::{sleep, Duration};
use futures_util::stream::{self};
use std::env;

fn add_cors_headers(res: &mut Response<Body>) {
    let headers = res.headers_mut();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert("Access-Control-Allow-Methods", "GET, POST, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type".parse().unwrap());
}

async fn router(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // Gestione della preflight request OPTIONS per CORS
    if req.method() == Method::OPTIONS {
        let mut response = Response::new(Body::empty());
        add_cors_headers(&mut response);
        return Ok(response);
    }

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

            let body = Body::wrap_stream(stream);
            let mut response = Response::new(body);
            add_cors_headers(&mut response);
            Ok(response)
        }
        (&Method::POST, "/upload") => {
            let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            let size = whole_body.len();
            println!("Ricevuti {} byte", size);
            let mut response = Response::new(Body::from("OK"));
            add_cors_headers(&mut response);
            Ok(response)
        }
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            add_cors_headers(&mut not_found);
            Ok(not_found)
        }
    }
}



#[tokio::main]
async fn main() {
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number");
    let addr = ([0, 0, 0, 0], port).into();

    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(router))
    });

    let server = Server::bind(&addr).serve(make_svc);
    println!("Server in ascolto su http://{}", addr);
    server.await.unwrap();
}

