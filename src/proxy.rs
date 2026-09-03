use std::{env, sync::Arc, time::Duration};

use axum::{
    body::StreamBody,
    extract::{Path, State},
    headers::CacheControl,
    response::IntoResponse,
    routing::get,
    Router, TypedHeader,
};
use tokio::sync::RwLock;

use crate::{
    helper::{self, PhixivError},
    state::PhixivState,
};

async fn proxy_handler(
    State(state): State<Arc<RwLock<PhixivState>>>,
    Path((path_first, path_rest)): Path<(String, String)>,
) -> Result<impl IntoResponse, PhixivError> {
    let state = state.read().await;

    let base = env::var("PXIMG_BASE").unwrap_or_else(|_| String::from("https://i.pximg.net/"));
    let url = format!("{base}{path_first}/{path_rest}");

    let mut headers = helper::headers();
    headers.append("Referer", "https://www.pixiv.net/".parse()?);

    let response = state.client.get(&url).headers(headers).send().await?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Forward upstream Content-Type; Discord rejects embed images without it.
    let mut header_map = http::HeaderMap::new();
    if let Some(ct) = content_type.and_then(|ct| ct.parse().ok()) {
        header_map.insert(http::header::CONTENT_TYPE, ct);
    }

    Ok((
        status,
        header_map,
        TypedHeader(
            CacheControl::new()
                .with_max_age(Duration::from_secs(60 * 60 * 24))
                .with_public(),
        ),
        StreamBody::new(response.bytes_stream()),
    ))
}

pub fn proxy_router(state: Arc<RwLock<PhixivState>>) -> Router<Arc<RwLock<PhixivState>>> {
    Router::new()
        .route("/:path_first/*path_rest", get(proxy_handler))
        .with_state(state)
}
