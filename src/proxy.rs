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

    // Upstream bases to try, in order: PXIMG_BASES (comma list, fallback chain)
    // then PXIMG_BASE (legacy single base), else i.pximg.net directly.
    let mut bases: Vec<String> = env::var("PXIMG_BASES")
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().trim_end_matches('/').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if bases.is_empty() {
        bases.push(
            env::var("PXIMG_BASE")
                .unwrap_or_else(|_| String::from("https://i.pximg.net")),
        );
    }
    let path_tail = format!("{path_first}/{path_rest}");

    let mut response = None;
    for base in &bases {
        let url = format!("{base}/{path_tail}");
        let mut headers = helper::headers();
        // External pximg proxy mirrors don't need (and some reject) the pixiv Referer.
        if base.contains("pximg.net") {
            headers.append("Referer", "https://www.pixiv.net/".parse()?);
        }
        match state.client.get(&url).headers(headers.clone()).send().await {
            Ok(r) if r.status().is_success() || r.status().as_u16() == 404 => {
                response = Some(r);
                break;
            }
            _ => continue,
        }
    }
    let response = response.ok_or_else(|| {
        anyhow::anyhow!("all image upstream proxies failed for /{path_tail}")
    })?;

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
