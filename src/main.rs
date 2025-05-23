use hyper::{
    Body, Request, Response, Server, Method, StatusCode,
    upgrade::Upgraded,
    header::{CONNECTION, UPGRADE, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_ACCEPT}
};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use serde_json::json;
use sha1::{Sha1, Digest};
use base64::engine::general_purpose;
use base64::Engine;
use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;


#[derive(Serialize, Deserialize, Clone)]
struct Measurement {
    timestamp: u64,
    ping_ms: f64,
}

type Store = Arc<Mutex<Vec<Measurement>>>;

fn add_cors_headers(res: &mut Response<Body>) {
    let headers = res.headers_mut();
    headers.insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    headers.insert("Access-Control-Allow-Methods", "GET, POST, OPTIONS".parse().unwrap());
    headers.insert("Access-Control-Allow-Headers", "Content-Type".parse().unwrap());
}

async fn handle_ws_connection(upgraded: Upgraded) {
    let ws_stream = WebSocketStream::from_raw_socket(
        upgraded,
        tokio_tungstenite::tungstenite::protocol::Role::Server,
        None
    ).await;

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {

                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                    if data.get("type") == Some(&Value::String("ping".into())) {
                        if let Some(seq) = data.get("seq") {
                            let pong_msg = json!({
                                "type": "pong",
                                "seq": seq
                            });

                            let pong_text = pong_msg.to_string();
                            if let Err(e) = ws_sender.send(Message::Text(pong_text)).await {
                                eprintln!("Errore invio pong WS: {}", e);
                                break;
                            }
                            continue; // saltare l'echo normale
                        }
                    }
                }

                if let Err(e) = ws_sender.send(Message::Text(text)).await {
                    eprintln!("Errore invio WS: {}", e);
                    break;
                }
            }
            Ok(Message::Binary(bin)) => {
                println!("Ricevuti dati binari di {} byte", bin.len());
            }
            Ok(Message::Close(_)) => {
                println!("Connessione WS chiusa dal client");
                break;
            }
            _ => {}
        }
    }
}

async fn router(req: Request<Body>, store: Store) -> Result<Response<Body>, Infallible> {
    if req.method() == Method::OPTIONS {
        let mut response = Response::new(Body::empty());
        add_cors_headers(&mut response);
        return Ok(response);
    }

    if req.uri().path() == "/ws" && req.headers().get(UPGRADE).map(|v| v == "websocket").unwrap_or(false) {
        if let Some(ws_key) = req.headers().get(SEC_WEBSOCKET_KEY) {
            let mut hasher = Sha1::new();
            hasher.update(ws_key.as_bytes());
            hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
            let result = hasher.finalize();
            let accept_key = general_purpose::STANDARD.encode(result);

            let response = Response::builder()
                .status(StatusCode::SWITCHING_PROTOCOLS)
                .header(CONNECTION, "Upgrade")
                .header(UPGRADE, "websocket")
                .header(SEC_WEBSOCKET_ACCEPT, accept_key)
                .body(Body::empty())
                .unwrap();

                
            tokio::spawn(async move {
                if let Ok(upgraded) = hyper::upgrade::on(req).await {
                    handle_ws_connection(upgraded).await;
                } else {
                    eprintln!("Errore nell'upgrade WebSocket");
                }
            });

            return Ok(response);
        } else {
            let mut response = Response::new(Body::from("Bad Request"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
        }
    }

    match (req.method(), req.uri().path()) {
        (&Method::POST, "/measure") => {
            let body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            match serde_json::from_slice::<Measurement>(&body) {
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
            let data = store.lock().await;
            let json_data = json!(*data);
            let body = Body::from(serde_json::to_string(&json_data).unwrap());
            let mut response = Response::new(body);
            response.headers_mut().insert("Content-Type", "application/json".parse().unwrap());
            add_cors_headers(&mut response);
            Ok(response)
        }
        (&Method::GET, "/download") => {
            let chunk_size = 1024 * 1024;
            let chunks_count = 100;

            use futures_util::stream;
            let stream = stream::unfold(0, move |count| async move {
                if count >= chunks_count {
                    None
                } else {
                    let chunk = vec![0u8; chunk_size];
                    Some((Ok::<Vec<u8>, Infallible>(chunk), count + 1))
                }
            });

            let body = Body::wrap_stream(stream);
            let mut response = Response::new(body);
            add_cors_headers(&mut response);
            Ok(response)
        }
        (&Method::POST, "/upload") => {
            let body = hyper::body::to_bytes(req.into_body()).await.unwrap();
            println!("Ricevuti {} byte", body.len());
            let mut response = Response::new(Body::from("OK"));
            add_cors_headers(&mut response);
            Ok(response)
        }
        _ => {
            let mut not_found = Response::new(Body::from("Not found"));
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            add_cors_headers(&mut not_found);
            Ok(not_found)
        }
    }
}

#[tokio::main]
async fn main() {
    let store: Store = Arc::new(Mutex::new(Vec::new()));

    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

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
    server.await.unwrap();
}
