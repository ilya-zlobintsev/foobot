use std::time::Duration;

use action_handler::spotify::SpotifyHandler;
use tokio::{task, time::sleep};

use crate::{
    action_handler,
    db::{DBConn, DBConnError},
};

#[derive(Clone)]
pub struct JobRunner {
    db_pool: DBConn,
    spotify_handler: SpotifyHandler,
}

impl JobRunner {
    pub fn new(db_pool: DBConn) -> Self {
        let spotify_handler = SpotifyHandler::new(
            db_pool.get_spotify_cilent_id().unwrap(),
            db_pool.get_spotify_client_secret().unwrap(),
        );
        JobRunner {
            db_pool,
            spotify_handler,
        }
    }

    pub async fn start(&self) -> Result<(), DBConnError> {
        let tokens = self.db_pool.get_spotify_refresh_tokens()?;

        for (channel, refresh_token) in tokens {
            self.start_spotify_token_refresh(channel, refresh_token)
                .await;
        }

        Ok(())
    }

    async fn start_spotify_token_refresh(&self, channel: String, refresh_token: String) {
        let db_pool = self.db_pool.clone();
        let spotify_handler = self.spotify_handler.clone();
        task::spawn(async move {
            loop {
                match spotify_handler.update_token(&refresh_token).await {
                    Ok((new_token, expiration_time)) => {
                        match db_pool.update_spotify_token(&channel, &new_token) {
                            Ok(()) => {
                                println!(
                                    "Updated spotify token for {}, next refresh in {}",
                                    channel, expiration_time
                                );
                                sleep(Duration::from_secs(expiration_time)).await;
                            }
                            Err(err) => {
                                println!(
                                    "DB error {:?} when updating spotify token for {}",
                                    err, channel
                                );
                                sleep(Duration::from_secs(60)).await;
                            }
                        };
                    }
                    Err(_) => {
                        println!("Error getting new spotify token for {}", channel);
                        sleep(Duration::from_secs(60)).await;
                    }
                };
            }
        });
    }
}
