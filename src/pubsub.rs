pub mod channel_points;

use std::time::Duration;

use channel_points::ChannelPointsRedeem;
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use serde_json::{json, Value};
use tokio::{task, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use twitch_irc::{login::StaticLoginCredentials, TCPTransport, TwitchIRCClient};

use crate::{
    action_handler::Action,
    command_handler::{Command, CommandHandler},
    db::DBConn,
    twitch_api::TwitchApi,
};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PubSubMessage {
    #[serde(rename = "type")]
    pub type_field: String,
    pub data: PubSubMessageData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PubSubMessageData {
    pub topic: String,
    pub message: String,
}

#[derive(Clone)]
pub struct PubSubHandler {
    twitch_api: TwitchApi,
    command_handler: CommandHandler,
    db_conn: DBConn,
}

impl PubSubHandler {
    pub fn new(twitch_api: TwitchApi, db_conn: DBConn) -> Self {
        let command_handler = CommandHandler::new(db_conn.clone(), twitch_api.clone());
        Self {
            twitch_api,
            command_handler,
            db_conn,
        }
    }

    pub async fn start(
        &self,
        channels: &Vec<String>,
        client: &TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) -> anyhow::Result<()> {
        let mut topics: Vec<String> = Vec::new();

        for user in self.twitch_api.get_users_by_login(channels).await?.data {
            topics.push(format!("community-points-channel-v1.{}", user.id));
        }

        println!("Connecting to pubsub for channels {:?}", topics);

        let (ws_stream, response) =
            connect_async(Url::parse("wss://pubsub-edge.twitch.tv").unwrap()).await?;
        let (mut write, mut read) = ws_stream.split();

        println!(
            "Pubsub connection established, status {}",
            response.status()
        );

        let auth = json!({
            "type": "LISTEN",
            "data": {
                "topics": topics,
                "auth_token": &self.twitch_api.get_oauth(),
            },
        });
        println!("Pubsub: using {}", auth.to_string());

        write.send(Message::Text(auth.to_string())).await?;

        task::spawn(async move {
            loop {
                println!("Pubsub: sending ping");
                match write.send(Message::Ping(vec![].into())).await {
                    Ok(_) => sleep(Duration::from_secs(60)).await,
                    Err(_) => break,
                };
            }
        });

        {
            let client = client.clone();
            let handler = self.clone();
            task::spawn(async move {
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(msg) => match msg {
                            Message::Ping(_) => println!("Pubsub: recieved PING"),
                            Message::Pong(_) => println!("Pubsub: recieved PONG"),
                            Message::Text(text) => {
                                // println!("Pubsub message: {}", text);
                                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                                    let handler = handler.clone();
                                    let client = client.clone();
                                    task::spawn(async move {
                                        handler.handle_msg(v, client).await.unwrap();
                                    });
                                }
                            }
                            _ => continue,
                        },
                        Err(e) => println!("Errror reading pubsub message: {:?}", e),
                    }
                }
                println!("Pubsub connection dropped");
            })
            .await?;
        }
        
        Ok(())
    }

    async fn handle_msg(
        &self,
        msg: Value,
        client: TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) -> anyhow::Result<()> {
        match msg["data"]["topic"].as_str() {
            Some(topic) => {
                if let Some(id) = topic.strip_prefix("community-points-channel-v1.") {
                    let channels = &self
                        .twitch_api
                        .get_users_by_id(&vec![id.to_string()])
                        .await?;
                    let channel = &channels.data.first().unwrap().login;
                    match serde_json::from_str::<ChannelPointsRedeem>(
                        msg["data"]["message"].as_str().unwrap(),
                    ) {
                        Ok(redeem) => {
                            if redeem.type_field == "reward-redeemed" {
                                println!(
                                    "Channel point redeem: #{} {}: {}",
                                    channel,
                                    redeem.data.redemption.user.login,
                                    redeem.data.redemption.reward.title
                                );

                                if let Some(action) = self.db_conn.get_points_redeem_trigger(
                                    &redeem.data.redemption.reward.title,
                                    channel,
                                )? {
                                    println!("Executing {}", action);

                                    let user_input =
                                        match &redeem.data.redemption.reward.is_user_input_required
                                        {
                                            true => redeem.data.redemption.user_input.unwrap(),
                                            false => String::new(),
                                        };
                                    let args: Vec<&str> = user_input.split_whitespace().collect();

                                    let response = self
                                        .command_handler
                                        .run_command(
                                            &Command {
                                                trigger: String::new(),
                                                action: Action::Custom(action),
                                                channel: channel.clone(),
                                                permissions:
                                                    crate::command_handler::Permissions::All,
                                            },
                                            &args,
                                            channel,
                                            client.clone(),
                                        )
                                        .await
                                        .unwrap();

                                    match response {
                                        Some(response) => {
                                            println!("Action executed, responding: {}", &response);
                                            client.say(channel.to_owned(), response).await?;
                                        }
                                        None => println!("Action executed, no output"),
                                    }
                                } else {
                                    println!("No action associated with redeem");
                                }
                            }
                        }
                        Err(e) => println!(
                            "Pubsub: failed to parse channel point redeems message {}",
                            e
                        ),
                    };
                }
            }
            None => println!("Pubsub: No topic in message"),
        }
        Ok(())
    }
}
