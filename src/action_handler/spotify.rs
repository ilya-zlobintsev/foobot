use std::collections::HashMap;

use reqwest::Client;
use serde_json::Value;

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

    pub async fn get_current_playlist(
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
            Ok(v) => Ok(Some(
                v["context"]["external_urls"]["spotify"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )),
            Err(e) => {
                println!("Error {:?} when getting the playlist", e);
                //Nothing is playing
                Ok(None)
            }
        }
    }

    pub async fn get_recently_played(&self, access_token: &str) -> Result<String, reqwest::Error> {
        match self
            .client
            .get("https://api.spotify.com/v1/me/player/recently-played")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?
            .json::<Value>()
            .await
        {
            Ok(recently_played) => {
                let last_track = &recently_played["items"][0]["track"];

                let artist = last_track["artists"][0]["name"].as_str().unwrap();
                let song = last_track["name"].as_str().unwrap();

                Ok(format!("{} - {}", artist, song))
            }
            Err(e) => Ok(format!("error getting last song: {:?}", e)),
        }
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
