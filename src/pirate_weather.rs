use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{error, info, instrument};

use crate::SharedState;

const CACHE_TTL_MS: i64 = chrono::Duration::hours(1).num_milliseconds();

pub struct Location {
    pub name: String,
    pub lat_lgn: (f64, f64),
    pub weather: RwLock<Option<Weather>>,
    pub last_cache_time_ms: RwLock<i64>,
}

impl Location {
    pub fn new(name: String, lat_lgn: (f64, f64)) -> Self {
        Self {
            name,
            lat_lgn,
            weather: None.into(),
            last_cache_time_ms: 0.into(),
        }
    }

    #[instrument(skip(self, pirate_weather_token))]
    pub async fn get_weather(&self, pirate_weather_token: &str) -> Result<Weather, String> {
        if self.last_cache_time_ms.read().await.clone()
            > chrono::Utc::now().timestamp_millis() - CACHE_TTL_MS
        {
            let weather_lock = self.weather.read().await;
            if weather_lock.is_none() {
                let mut last_cache_time_ms = self.last_cache_time_ms.write().await;
                error!(
                    location = self.name,
                    last_cache_time_ms = last_cache_time_ms.clone(),
                    "Cached weather are None but TTL set"
                );

                *last_cache_time_ms = 0;
                return Err("internal".to_string());
            }
            info!("Returning cached weather");
            return Ok(weather_lock.as_ref().unwrap().clone());
        }

        let client = reqwest::Client::new();
        let res = client
            .get(format!(
                "https://api.pirateweather.net/forecast/{}/{},{}",
                pirate_weather_token, self.lat_lgn.0, self.lat_lgn.1
            ))
            .query(&[("units", "uk")])
            .send()
            .await
            .unwrap();
        let body = res.text().await;

        if let Err(err) = body {
            error!(%err, "failed to convert body to text");
            return Err("internal".to_string());
        }

        let body = body.unwrap();

        let json_body = serde_json::from_str(&body);
        if let Err(err) = json_body {
            error!(
                %err,
                location = self.name,
                "failed to parse body"
            );
            return Err("internal".to_string());
        }

        let parsed_body: Weather = json_body.unwrap();
        *self.last_cache_time_ms.write().await = chrono::Utc::now().timestamp_millis();

        self.weather.write().await.replace(parsed_body.clone());

        info!(location = self.name, "Returning unached weather");
        Ok(parsed_body)
    }
}

#[instrument(skip(state))]
pub async fn get_location(
    State(state): State<SharedState>,
) -> Result<Json<LocationResponse>, String> {
    info!("fetching weather");
    let current_location = state.location.read().await;
    let weather = current_location
        .get_weather(&state.priate_weather_token)
        .await?;

    Ok(Json(LocationResponse {
        location: current_location.name.clone(),
        latitude: current_location.lat_lgn.0,
        longitude: current_location.lat_lgn.1,
        timezone: weather.timezone,
        offset: weather.offset,
        elevation: weather.elevation,
        currently: weather.currently,
    }))
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationResponse {
    pub location: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub offset: f64,
    pub elevation: i64,
    pub currently: Currently,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub offset: f64,
    pub elevation: i64,
    pub currently: Currently,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currently {
    pub time: i64,
    pub summary: String,
    pub icon: String,
    pub nearest_storm_distance: f64,
    pub nearest_storm_bearing: f64,
    pub precip_intensity: f64,
    pub precip_probability: f64,
    pub precip_intensity_error: f64,
    pub precip_type: String,
    pub temperature: f64,
    pub apparent_temperature: f64,
    pub dew_point: f64,
    pub humidity: f64,
    pub pressure: f64,
    pub wind_speed: f64,
    pub wind_gust: f64,
    pub wind_bearing: f64,
    pub cloud_cover: f64,
    pub uv_index: f64,
    pub visibility: f64,
    pub ozone: f64,
}
