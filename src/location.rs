use axum::extract::State;
use axum::http::{status, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::Error;
use tracing::{error, instrument};
use tracing::{info, warn};

use crate::SharedState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedLocation {
    pub name: String,
    pub lat_lgn: (f64, f64),
}

impl Default for SavedLocation {
    fn default() -> Self {
        Self {
            name: "London".into(),
            lat_lgn: (51.510803, -0.120703),
        }
    }
}

impl Into<crate::pirate_weather::Location> for SavedLocation {
    fn into(self) -> crate::pirate_weather::Location {
        crate::pirate_weather::Location::new(self.name, self.lat_lgn)
    }
}

fn get_setting_path() -> String {
    std::env::var("SETTING_PATH").unwrap_or("location.json".into())
}

impl SavedLocation {
    #[instrument]
    pub async fn load_location() -> Result<SavedLocation, anyhow::Error> {
        if tokio::fs::metadata(get_setting_path()).await.is_err() {
            info!("location file not found defaulting to London");

            let location = SavedLocation::default();

            if let Err(err) = tokio::fs::write(
                get_setting_path(),
                serde_json::to_string(&location).unwrap(),
            )
            .await
            {
                error!(%err, "Failed to write default location file");
            }

            return Ok(location);
        }

        let location = tokio::fs::read_to_string(get_setting_path()).await;
        if let Err(err) = location {
            error!(%err,"Failed to read location file");
            return Err(err.into());
        }

        let location: Result<SavedLocation, Error> = serde_json::from_str(&location.unwrap());
        if let Err(err) = location {
            error!(%err, "Failed to parse location file writing default" );
            let location = SavedLocation::default();

            if let Err(err) = SavedLocation::save_location(&location).await {
                error!(%err, "Failed to write default location file");
            }

            return Ok(location);
        }

        let parsed_location = location.unwrap();
        info!(?parsed_location, "Loaded location from file");
        Ok(parsed_location)
    }

    #[instrument]
    pub async fn save_location(location: &SavedLocation) -> Result<(), anyhow::Error> {
        let location = serde_json::to_string(location).unwrap();
        tokio::fs::write(get_setting_path(), location).await?;
        Ok(())
    }
}

#[instrument(skip(state, headers))]
pub async fn set_location(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<SavedLocation>,
) -> impl IntoResponse {
    let auth_header = headers
        .get("authorization")
        .and_then(|header| header.to_str().ok());

    if auth_header.is_none() || state.admin_auth_token != auth_header.unwrap() {
        info!(
            "Failed to connect auth header is {}",
            if auth_header.is_none() {
                "missing"
            } else {
                "invalid"
            }
        );
        warn!("unable to set location without valid auth token");
        return status::StatusCode::UNAUTHORIZED;
    }

    if let Err(err) = SavedLocation::save_location(&body).await {
        error!(%err, "Failed to save location");
        return status::StatusCode::INTERNAL_SERVER_ERROR;
    }

    info!("setting the location");
    *state.location.write().await = body.into();
    return status::StatusCode::OK;
}
