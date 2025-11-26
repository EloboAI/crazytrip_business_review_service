use serde::Serialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize)]
pub struct SharePromotionRequest {
    pub author_id: Uuid,
    pub author_name: Option<String>,
    pub promotion_id: Uuid,
    pub business_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub media_urls: Vec<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct StoriesClient {
    client: reqwest::Client,
    base_url: String,
}

impl StoriesClient {
    pub fn new(base_url: String) -> Self {
        let normalized = normalize_base_url(&base_url);
        Self {
            client: reqwest::Client::new(),
            base_url: normalized,
        }
    }

    pub async fn share_promotion(&self, request: SharePromotionRequest) -> Result<(), String> {
        let url = format!("{}/stories/promotion", self.base_url);
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("Failed to share promotion: {}", text));
        }

        Ok(())
    }
}

fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim_end_matches('/');
    if trimmed.ends_with("/api/v1") {
        trimmed.to_string()
    } else {
        format!("{}/api/v1", trimmed)
    }
}
