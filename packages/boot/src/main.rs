mod routes;
mod utils;

use anyhow::Result;
use hyper::Method;
use log::info;
use std::net::SocketAddr;
use tracing::Span;

use axum::{body::Body, http::Request, serve};
use tokio::net::TcpListener;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use yew::platform::Runtime;

use crate::routes::route;
use _database::{
    functions::api::log::user::insert_user_log,
    init,
    types::response::api::log::{UserLogItemOperation, UserLogVisitMethod},
};

#[derive(Clone, Default)]
struct Executor {
    inner: Runtime,
}

impl<F> hyper::rt::Executor<F> for Executor
where
    F: std::future::Future + Send + 'static,
{
    fn execute(&self, fut: F) {
        self.inner.spawn_pinned(move || async move {
            fut.await;
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(not(debug_assertions))]
    {
        use tracing_subscriber::fmt::writer::MakeWriterExt;

        let file_appender =
            tracing_appender::rolling::daily(_database::LOG_DIR.clone(), "kongying-server-logs");
        let std_out = std::io::stdout.with_max_level(tracing::Level::WARN);
        tracing_subscriber::fmt()
            .with_writer(std_out.and(file_appender))
            .init();
    }

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(std::io::stdout)
            .init();
    }

    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80);

    init().await?;

    let router = route()
        .await?
        .layer(
            TraceLayer::new_for_http().on_request(|req: &Request<Body>, _span: &Span| {
                let uri = req.uri().to_string();

                // 仅当开头为 `/api/` 时才记录日志
                if uri.starts_with("/api/") {
                    let ip = req
                        .headers()
                        .get("X-Real-IP")
                        .and_then(|ip| {
                            ip.to_str()
                                .ok()
                                .map(|ip| ip.parse::<SocketAddr>().ok())
                                .flatten()
                        })
                        .map(|addr| addr.ip());
                    let user_agent = req
                        .headers()
                        .get("User-Agent")
                        .map(|ua| ua.to_str().unwrap_or("Empty").to_string());

                    let method = match *req.method() {
                        Method::GET => UserLogVisitMethod::Get,
                        Method::POST => UserLogVisitMethod::Post,
                        Method::PUT => UserLogVisitMethod::Put,
                        Method::DELETE => UserLogVisitMethod::Delete,
                        _ => return,
                    };
                    let uri = uri[4..].to_string();

                    let _ =
                        insert_user_log(UserLogItemOperation::Visit((method, uri)), ip, user_agent);
                }
            }),
        )
        .layer(CompressionLayer::new())
        .into_make_service_with_connect_info::<SocketAddr>();

    info!("Site will run on port {port}");

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");
    serve(listener, router).await?;

    Ok(())
}
