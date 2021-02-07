use std::time::Duration;

use spotify::SpotifyHandler;
use tokio::{task, time::sleep};
use translate::TranslationHandler;
use twitch_irc::{login::StaticLoginCredentials, TCPTransport, TwitchIRCClient};
use weather::WeatherHandler;

use crate::{
    command_handler::CommandHandlerError,
    db::{DBConn, DBConnError},
    twitch_api::TwitchApi,
};

use self::weather::WeatherError;
pub mod spotify;
mod sys;
pub mod translate;
pub mod weather;

#[derive(Clone, Debug)]
pub enum Action {
    Custom(String),
    AddCmd,
    DelCmd,
    ShowCmd,
    ListCmd,
    Join,
    Part,
}

#[derive(Clone)]
pub struct ActionHandler {
    db_conn: DBConn,
    twitch_api: TwitchApi,
    weather_handler: WeatherHandler,
    spotify_handler: SpotifyHandler,
    translator: TranslationHandler,
}

impl ActionHandler {
    pub fn new(db_conn: DBConn, twitch_api: TwitchApi) -> Self {
        let weather_handler = WeatherHandler::new(db_conn.get_openweathermap_api_key().unwrap());
        let translator = TranslationHandler::new();
        let spotify_handler = SpotifyHandler::new(
            db_conn.get_spotify_cilent_id().unwrap(),
            db_conn.get_spotify_client_secret().unwrap(),
        );

        Self {
            db_conn,
            twitch_api,
            weather_handler,
            translator,
            spotify_handler,
        }
    }

    pub async fn run(
        &self,
        action: &str,
        args: &Vec<String>,
        channel: &str,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) -> Result<Option<String>, CommandHandlerError> {
        println!("Executing action {} with arguments {:?}", action, args);

        match action {
            "spotify" => Ok(Some(self.get_spotify(channel).await?)),
            "lastsong" => Ok(Some(self.get_spotify_last_song(channel).await?)),
            "hitman" => Ok(Some(
                self.hitman(channel, args.first().unwrap(), client).await?,
            )),
            "bodyguard" => Ok(Some(
                self.bodyguard(channel, args.first().unwrap(), client)
                    .await?,
            )),
            "ping" => Ok(Some(sys::SysInfo::ping())),
            "commercial" => Ok(Some(
                self.run_ad(channel, args.first().unwrap().parse().unwrap())
                    .await?,
            )),
            "weather" => Ok(match args.is_empty() {
                false => Some(self.get_weather(&args.join(" ")).await?),
                true => Some(String::from("location not specified")),
            }),
            "translate" => Ok(Some(self.translate(args.first().unwrap()).await?)),
            "emoteonly" => match args.first().unwrap().parse::<u64>() {
                Ok(duration) => {
                    self.emote_only(channel, duration, client).await;
                    Ok(None)
                }
                Err(_) => Err(CommandHandlerError::ExecutionError(String::from(
                    "invalid duration",
                ))),
            },
            _ => Err(CommandHandlerError::ExecutionError(format!(
                "unknown action {}",
                action
            ))),
        }
    }

    async fn get_spotify(&self, channel: &str) -> Result<String, CommandHandlerError> {
        match self.db_conn.get_spotify_access_token(channel) {
            Ok((access_token, _)) => {
                match self.spotify_handler.get_current_song(&access_token).await? {
                    Some(song) => Ok(song),
                    None => Ok(String::from("no song is currently playing")),
                }
            }
            Err(e) => match e {
                DBConnError::NotFound => Ok(String::from("not configured for this channel")),
                _ => Err(CommandHandlerError::DBError(e)),
            },
        }
    }

    async fn get_spotify_last_song(&self, channel: &str) -> Result<String, CommandHandlerError> {
        match self.db_conn.get_spotify_access_token(channel) {
            Ok((access_token, _)) => {
                match self
                    .spotify_handler
                    .get_recently_played(&access_token)
                    .await
                {
                    Ok(recently_played) => {
                        let last_track = &recently_played.items.first().unwrap().track;

                        Ok(format!(
                            "{} - {}",
                            last_track.artists.first().unwrap().name,
                            last_track.name
                        ))
                    }
                    Err(e) => Ok(format!("error getting last song: {:?}", e)),
                }
            }
            Err(e) => match e {
                DBConnError::NotFound => Ok(String::from("not configured for this channel")),
                _ => Err(CommandHandlerError::DBError(e)),
            },
        }
    }

    async fn hitman(
        &self,
        channel: &str,
        user: &str,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) -> Result<String, CommandHandlerError> {
        self.db_conn.add_hitman(channel, user)?;

        client
            .say(
                channel.to_owned(),
                format!("Timing out {} in 15 seconds...", user),
            )
            .await
            .expect("failed to say");

        sleep(Duration::from_secs(15)).await;

        match self.db_conn.get_hitman_protected(user, channel)? {
            false => {
                client
                    .privmsg(channel.to_owned(), format!("/timeout {} 600", user))
                    .await
                    .expect("failed to time out");
                self.db_conn.set_hitman_protection(user, channel, &false)?;

                Ok(format!("{} timed out for 10 minutes!", user))
            }
            true => {
                self.db_conn.set_hitman_protection(user, channel, &false)?;

                Ok(String::new())
            }
        }
    }

    async fn bodyguard(
        &self,
        channel: &str,
        user: &str,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) -> Result<String, CommandHandlerError> {
        self.db_conn.set_hitman_protection(user, channel, &true)?;
        client
            .say(channel.to_owned(), format!("{} has been guarded!", user))
            .await
            .expect("failed to say");

        Ok(String::new())
    }

    async fn emote_only(
        &self,
        channel: &str,
        duration: u64,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) {
        client
            .say(
                channel.to_string(),
                format!("Emote-only enabled for {} seconds!", duration),
            )
            .await
            .expect("Failed to say");

        client
            .privmsg(channel.to_string(), "/emoteonly".to_string())
            .await
            .expect("Failed to write");
        sleep(Duration::from_secs(duration)).await;
        client
            .privmsg(channel.to_string(), "/emoteonlyoff".to_string())
            .await
            .expect("Failed to write");
    }

    async fn run_ad(&self, channel: &str, duration: u8) -> Result<String, CommandHandlerError> {
        if duration == 60 || duration == 120 || duration == 180 {
            self.twitch_api.run_ad(channel, duration).await?;
            Ok(format!("Running an ad for {} seconds", duration))
        }
        else {
            Ok(String::from("Invalid ad duration"))
        }
    }

    async fn get_weather(&self, location: &str) -> Result<String, CommandHandlerError> {
        match self.weather_handler.get_weather(location.to_owned()).await {
            Ok(weather) => Ok(format!(
                "{}, {}: {}°C, {}",
                weather.name,
                weather.sys.country.unwrap_or_default(),
                weather.main.temp,
                weather.weather.first().unwrap().description
            )),
            Err(e) => match e {
                WeatherError::InvalidLocation => Ok(String::from("location not found")),
                _ => Ok(format!("Failed getting weather: {:?}", e)),
            },
        }
    }

    async fn translate(&self, text: &str) -> Result<String, CommandHandlerError> {
        match self.translator.translate(text).await {
            Ok(translation) => Ok(format!(
                "{} -> {}: {}",
                translation.src, translation.dest, translation.text
            )),
            Err(e) => Ok(format!("error when translating: {:?}", e)),
        }
    }
}
