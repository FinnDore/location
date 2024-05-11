use serde_json::Error;
use tracing::info;
use tracing::{error, instrument};

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

impl SavedLocation {
    #[instrument]
    pub async fn load_location() -> Result<SavedLocation, anyhow::Error> {
        if tokio::fs::metadata("location.json").await.is_err() {
            info!("location file not found defaulting to London");

            let location = SavedLocation::default();

            if let Err(err) =
                tokio::fs::write("location.json", serde_json::to_string(&location).unwrap()).await
            {
                error!(%err, "Failed to write default location file");
            }

            return Ok(location);
        }

        let location = tokio::fs::read_to_string("location.json").await;
        if let Err(err) = location {
            error!(%err,"Failed to read location file");
            return Err(err.into());
        }

        let location: Result<SavedLocation, Error> = serde_json::from_str(&location.unwrap());
        if let Err(err) = location {
            error!(%err, "Failed to parse location file writing default" );
            let location = SavedLocation::default();

            if let Err(err) =
                tokio::fs::write("location.json", serde_json::to_string(&location).unwrap()).await
            {
                error!(%err, "Failed to write default location file");
            }

            return Ok(location);
        }

        let parsed_location = location.unwrap();
        info!(?parsed_location, "Loaded location from file");
        Ok(parsed_location)
    }
}
