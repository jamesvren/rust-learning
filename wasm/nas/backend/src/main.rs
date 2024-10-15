use axum::{
    Router,
    routing::get,
    response::{IntoResponse, Response},
    body::Body,
    http::StatusCode,
};
use tokio::net::TcpListener;
use tokio::fs;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use std::path::PathBuf;


#[tokio::main]
async fn main() {
    // Setup logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", format!("{},hyper=info,mio=info", "debug"));
    }
    // Enable console logging
    tracing_subscriber::fmt::init();

    let app = Router::new()
        //.route("/", get(hello))
        .route("/api/hello", get(hello))
        .fallback_service(get(|req| async move {
            match ServeDir::new("./frontend/dist").oneshot(req).await {
                Ok(res) => {
                    let status = res.status();
                    match status {
                        StatusCode::NOT_FOUND => {
                            let index_path = PathBuf::from("./frontend/dist/index.html");
                            let index_content = match fs::read_to_string(index_path).await {
                                Err(_) => {
                                    return Response::builder()
                                        .status(StatusCode::NOT_FOUND)
                                        .body(Body::from("index file not found"))
                                        .unwrap()
                                }
                                Ok(index_content) => index_content,
                            };

                            Response::builder()
                                .status(StatusCode::OK)
                                .body(Body::from(index_content))
                                .unwrap()
                        }
                        _ => res.into_response(),
                    }
                }
                Err(err) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!("error: {err}")))
                    .expect("error response"),
            }
        }))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    log::info!("Server started on http:://0.0.0.0:3000");

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> impl IntoResponse {
    "Hello from server!"
}
