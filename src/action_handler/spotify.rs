use std::collections::HashMap;

use reqwest::Client;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRecentlyPlayed {
    pub items: Vec<Item>,
    pub next: String,
    pub cursors: Cursors,
    pub limit: i64,
    pub href: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub track: Track,
    #[serde(rename = "played_at")]
    pub played_at: String,
    pub context: Option<Context>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub album: Album,
    pub artists: Vec<Artist2>,
    #[serde(rename = "available_markets")]
    pub available_markets: Vec<String>,
    #[serde(rename = "disc_number")]
    pub disc_number: i64,
    #[serde(rename = "duration_ms")]
    pub duration_ms: i64,
    pub explicit: bool,
    #[serde(rename = "external_ids")]
    pub external_ids: ExternalIds,
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls4,
    pub href: String,
    pub id: String,
    #[serde(rename = "is_local")]
    pub is_local: bool,
    pub name: String,
    pub popularity: i64,
    #[serde(rename = "preview_url")]
    pub preview_url: String,
    #[serde(rename = "track_number")]
    pub track_number: i64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    #[serde(rename = "album_type")]
    pub album_type: String,
    pub artists: Vec<Artist>,
    #[serde(rename = "available_markets")]
    pub available_markets: Vec<String>,
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls2,
    pub href: String,
    pub id: String,
    pub images: Vec<Image>,
    pub name: String,
    #[serde(rename = "release_date")]
    pub release_date: String,
    #[serde(rename = "release_date_precision")]
    pub release_date_precision: String,
    #[serde(rename = "total_tracks")]
    pub total_tracks: i64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls,
    pub href: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls2 {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub height: i64,
    pub url: String,
    pub width: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist2 {
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls3,
    pub href: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls3 {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalIds {
    pub isrc: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls4 {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    #[serde(rename = "external_urls")]
    pub external_urls: ExternalUrls5,
    pub href: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub uri: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalUrls5 {
    pub spotify: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursors {
    pub after: String,
    pub before: String,
}

#[derive(Clone)]
pub struct SpotifyHandler {
    client_id: String,
    client_secret: String,
    client: reqwest::Client,
}

impl SpotifyHandler {
    pub fn new(client_id: String, client_secret: String) -> SpotifyHandler {
        SpotifyHandler {
            client_id,
            client_secret,
            client: Client::new(),
        }
    }

    pub async fn get_current_song(
        &self,
        access_token: &str,
    ) -> Result<Option<String>, reqwest::Error> {
        let response = self
            .client
            .get("https://api.spotify.com/v1/me/player")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        match response.json::<Value>().await {
            Ok(v) => {
                if let Some(error) = v.get("error") {
                    Ok(Some(format!("error: {}", error.get("message").unwrap())))
                } else {
                    let title = v["item"]["name"].as_str().unwrap();

                    let mut artists: Vec<&str> = Vec::new();
                    for artist in v["item"]["artists"].as_array().unwrap() {
                        artists.push(artist["name"].as_str().unwrap());
                    }
                    let artists = artists.join(", ");

                    let position = v["progress_ms"].as_u64().unwrap() / 1000;
                    let position = format!("{}:{:02}", position / 60, position % 60);

                    let length = v["item"]["duration_ms"].as_u64().unwrap() / 1000;
                    let length = format!("{}:{:02}", length / 60, length % 60);

                    Ok(Some(format!(
                        "{} - {} [{}/{}]",
                        artists, title, position, length
                    )))
                }
            }
            Err(_) => {
                //Nothing is playing
                Ok(None)
            }
        }
    }

    pub async fn get_recently_played(
        &self,
        access_token: &str,
    ) -> Result<UserRecentlyPlayed, reqwest::Error> {
        Ok(self
            .client
            .get("https://api.spotify.com/v1/me/player/recently-played")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?
            .json()
            .await?)
    }

    ///Returns new access token and the expiration time
    pub async fn update_token(&self, refresh_token: &str) -> Result<(String, u64), reqwest::Error> {
        let mut payload: HashMap<&str, &str> = HashMap::new();
        payload.insert("grant_type", "refresh_token");
        payload.insert("refresh_token", refresh_token);
        payload.insert("redirect_uri", "http://localhost:5555/");
        payload.insert("client_id", &self.client_id);
        payload.insert("client_secret", &self.client_secret);

        let response: Value = self
            .client
            .post("https://accounts.spotify.com/api/token")
            .form(&payload)
            .send()
            .await?
            .json()
            .await?;

        // println!("{:?}", response);

        Ok((
            response["access_token"].as_str().unwrap().to_string(),
            response["expires_in"].as_u64().unwrap(),
        ))
    }
}
