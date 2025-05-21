use hyper::{Body, Request, Response, Server, Method, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::env;
use serde::{Serialize, Deserialize};
use serde_json::json;

fn add_cors_headers(res: &mut Response<Body>) {
    let headers = res.headers_mut();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert("Access-Control-Allow-Methods", "GET, POST, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type".parse().unwrap());
}

#[derive(Serialize, Deserialize, Clone)]
struct Measurement {
    timestamp: u64, // es. epoch ms dal client
    ping_ms: f64,
    // eventualmente aggiungi jitter, packet_loss, ecc.
}

type Store = Arc<Mutex<Vec<Measurement>>>;

async fn router(req: Request<Body>, store: Store) -> Result<Response<Body>, Infallible> {
    if req.method() == Method::OPTIONS {
        let mut response = Response::new(Body::empty());
        add_cors_headers(&mut response);
        return Ok(response);
    }

    match (req.method(), req.uri().path()) {
        (&Method::POST, "/measure") => {
            // Riceve una singola misurazione in JSON e la salva
            let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            match serde_json::from_slice::<Measurement>(&whole_body) {
                Ok(measure) => {
                    let mut data = store.lock().await;
                    data.push(measure);
                    let mut response = Response::new(Body::from("Measurement saved"));
                    add_cors_headers(&mut response);
                    Ok(response)
                }
                Err(_) => {
                    let mut response = Response::new(Body::from("Invalid JSON"));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    add_cors_headers(&mut response);
                    Ok(response)
                }
            }
        }
        (&Method::GET, "/measures") => {
            // Restituisce tutte le misurazioni in JSON
            let data = store.lock().await;
            let json_data = json!(*data);
            let body = Body::from(serde_json::to_string(&json_data).unwrap());
            let mut response = Response::new(body);
            response.headers_mut().insert("Content-Type", "application/json".parse().unwrap());
            add_cors_headers(&mut response);
            Ok(response)
        }
        (&Method::GET, "/download") => {
            // come prima, simulazione download
            let chunk_size = 1024 * 1024;
            let chunks_count = 100;

            use futures_util::stream;
            let stream = stream::unfold(0, move |count| async move {
                if count >= chunks_count {
                    None
                } else {
                    let chunk = vec![0u8; chunk_size];
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
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
    let store: Store = Arc::new(Mutex::new(Vec::new()));

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number");
    let addr = ([0, 0, 0, 0], port).into();

    let make_svc = make_service_fn(move |_conn| {
        let store = store.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                router(req, store.clone())
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    println!("Server in ascolto su http://{}", addr);
    server.await.unwrap();
}
