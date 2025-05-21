use hyper::{Body, Request, Response, Server, Method, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use tokio::time::{sleep, Duration};
use futures_util::stream::{self};
use std::env;


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

            let body = Body::wrap_stream(stream);
            Ok(Response::new(body))
        }
        (&Method::POST, "/upload") => {
            let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            let size = whole_body.len();
            println!("Ricevuti {} byte", size);
            Ok(Response::new(Body::from("OK")))
        }
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
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

