mod location;
mod pirate_weather;

use axum::http::HeaderValue;
use axum::routing::{get, post};
use location::SavedLocation;
use pirate_weather::Location;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::instrument;
use tracing::{info, level_filters::LevelFilter};

use axum::Router;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{fmt, prelude::*, Registry};

use crate::location::set_location;
use crate::pirate_weather::get_location;

pub struct TheState {
    pub admin_auth_token: String,
    pub priate_weather_token: String,
    pub location: Arc<RwLock<Location>>,
    pub contributions_last_cache_time_ms: Arc<RwLock<i64>>,
}

impl TheState {
    pub fn new(
        priate_weather_token: String,
        admin_auth_token: String,
        saved_location: SavedLocation,
    ) -> Self {
        Self {
            priate_weather_token,
            admin_auth_token,
            location: Arc::new(RwLock::new(Location::new(
                saved_location.name,
                saved_location.lat_lgn,
            ))),
            contributions_last_cache_time_ms: Arc::new(0.into()),
        }
    }
}

pub type SharedState = Arc<TheState>;

#[tokio::main]
#[instrument]
async fn main() {
    let env = std::env::var("ENV").unwrap_or("production".into());
    if env == "development" {
        tracing_subscriber::fmt().without_time().init();
    } else {
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env()
            .expect("Failed to create env filter invalid RUST_LOG env var");

        let registry = Registry::default().with(env_filter).with(fmt::layer());

        if let Ok(_) = std::env::var("AXIOM_TOKEN") {
            let axiom_layer = tracing_axiom::builder()
                .with_service_name("location")
                .with_tags(&[(
                    &"deployment_id",
                    &std::env::var("RAILWAY_DEPLOYMENT_ID")
                        .map(|s| {
                            s + "-"
                                + std::env::var("RAILWAY_DEPLOYMENT_ID")
                                    .unwrap_or("unknown_replica".into())
                                    .as_str()
                        })
                        .unwrap_or("unknown_deployment".into()),
                )])
                .with_tags(&[(&"service.name", "location".into())])
                .layer()
                .expect("Axiom layer failed to initialize");

            registry
                .with(axiom_layer)
                .try_init()
                .expect("Failed to initialize tracing with axiom");
            info!("Initialized tracing with axiom");
        } else {
            registry.try_init().expect("Failed to initialize tracing");
        }
    };

    let pirate_weather_token =
        std::env::var("PIRATE_WEATHER_TOKEN").expect("PIRATE_WEATHER_TOKEN env var not set");
    let auth_token = std::env::var("AUTH_TOKEN").expect("AUTH_TOKEN env var not set");
    let location = SavedLocation::load_location()
        .await
        .map_err(|_| SavedLocation::default());

    let state = Arc::new(TheState::new(
        pirate_weather_token,
        auth_token,
        location.unwrap(),
    ));

    let app = Router::new()
        .route("/location", get(get_location))
        .route("/location", post(set_location))
        .layer(CorsLayer::new().allow_origin([
            "https://finndore.dev".parse().unwrap(),
            "https://*finnnn.vercel.app".parse().unwrap(),
            "http://localhost:3000".parse().unwrap(),
        ]))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or("3002".to_string());
    let host = format!("0.0.0.0:{}", port);
    info!("Running server on {}", host);

    let listener = tokio::net::TcpListener::bind(host).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
