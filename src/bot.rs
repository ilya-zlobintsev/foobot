use std::{collections::HashMap, time::Duration};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task,
    time::sleep,
};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, TCPTransport, TwitchIRCClient,
};

use crate::{
    command_handler::{CommandHandler, CommandHandlerError, Permissions},
    db::DBConn,
    jobs::JobRunner,
    pubsub::PubSubHandler,
    twitch_api::TwitchApi,
};

#[derive(Debug)]
pub struct BotConfig {
    pub login: StaticLoginCredentials,
    pub oauth: String,
    pub channels: Vec<String>,
    pub prefixes: HashMap<String, String>,
}

pub struct Bot {
    config: BotConfig,
    command_handler: CommandHandler,
    pubsub_handler: PubSubHandler,
    job_runner: JobRunner,
}

impl Bot {
    pub async fn new(db_conn: DBConn) -> anyhow::Result<Self> {
        let config = db_conn.get_bot_config()?;

        let twitch_api = TwitchApi::init(&config.oauth).await?;

        let command_handler = CommandHandler::new(db_conn.clone(), twitch_api.clone());
        let pubsub_handler =
            PubSubHandler::new(TwitchApi::init(&config.oauth).await?, db_conn.clone());
        let job_runner = JobRunner::new(db_conn);

        Ok(Bot {
            config,
            command_handler,
            pubsub_handler,
            job_runner,
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let channels = self.config.channels.clone();
        let job_runner = self.job_runner.clone();
        let pubsub_handler = self.pubsub_handler.clone();

        let config = ClientConfig::new_simple(self.config.login.to_owned());
        let (mut incoming_messages, client) =
            TwitchIRCClient::<TCPTransport, StaticLoginCredentials>::new(config);

        let (msg_sender, msg_receiver) = mpsc::channel(1000);
        {
            let client = client.clone();
            task::spawn(async move { Self::listen_msg(msg_receiver, client).await });
        }

        let msg_sender1 = msg_sender.clone();

        let join_handle = task::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                let msg_sender = msg_sender1.clone();

                match message {
                    ServerMessage::Privmsg(privmsg) => {
                        // println!("#{} {}: {}", privmsg.channel_login, privmsg.sender.name, privmsg.message_text);
                        self.handle_privmsg(privmsg, msg_sender)
                    }
                    _ => continue,
                }
            }
        });

        for channel in &channels {
            println!("Joining {}", channel);
            client.join(channel.to_owned());
            msg_sender
                .send(SendMsg::Say((
                    channel.to_owned(),
                    "AlienPls Bot started AlienPls".to_owned(),
                )))
                .await
                .expect("Failed writing to sender");
        }

        // let quit_handle = runner.quit_handle();

        job_runner.start().await?;

        task::spawn(async move {
            loop {
                pubsub_handler.start(&channels, msg_sender.clone()).await;
                println!("Pubsub: reconnecting...");
                sleep(Duration::from_secs(5)).await;
            }
        });

        join_handle.await?;

        Ok(())
    }

    fn handle_privmsg(&self, pm: PrivmsgMessage, msg_sender: Sender<SendMsg>) {
        if let Some(cmd) = self.parse_command(&pm.message_text, &pm.channel_login) {
            println!(
                "{} {}: {}",
                pm.channel_login, pm.sender.login, pm.message_text
            );
            let command_handler = self.command_handler.clone();

            task::spawn(async move {
                let split = cmd.split_whitespace().collect::<Vec<&str>>();
                let (trigger, arguments) = split.split_first().unwrap();

                match command_handler.get_command(trigger, &pm.channel_login) {
                    //If the database response was proper
                    Ok(cmd) => {
                        //If the command exists
                        if let Some(cmd) = cmd {
                            if Self::check_command_permissions(&pm, &cmd.permissions) {
                                println!("Executing {:?}", cmd.action);

                                match command_handler
                                    .run_command(
                                        &cmd,
                                        arguments,
                                        &pm.channel_login,
                                        &pm.sender.login,
                                        msg_sender.clone(),
                                    )
                                    .await
                                {
                                    Ok(execution) => {
                                        if let Some(response) = execution {
                                            println!("Responding with {}", response);

                                            msg_sender
                                                .send(SendMsg::Reply((response, pm)))
                                                .await
                                                .expect("Failed writing message to sender");
                                        } else {
                                            println!("Execution finished (no response)");
                                        }
                                    }
                                    Err(err) => match err {
                                        CommandHandlerError::ExecutionError(err) => {
                                            msg_sender
                                                .send(SendMsg::Reply((
                                                    format!("Execution error error: {}", err),
                                                    pm,
                                                )))
                                                .await
                                                .expect("Failed to send");
                                        }
                                        CommandHandlerError::DBError(err) => {
                                            msg_sender
                                                .send(SendMsg::Reply((
                                                    format!("DB error: {}", err),
                                                    pm,
                                                )))
                                                .await
                                                .expect("Failed to send");
                                        }
                                        _ => unreachable!(),
                                    },
                                }
                            } else {
                                msg_sender
                                    .send(SendMsg::Reply((
                                        "you do not have the permissions to use this command!"
                                            .to_string(),
                                        pm,
                                    )))
                                    .await
                                    .expect("Failed to send")
                            }
                        } else {
                            println!("Command {} not found", trigger);
                        }
                    }
                    Err(err) => {
                        println!("Error while getting command: {:?}", err);
                    }
                }
            });
        }
    }

    fn check_command_permissions(pm: &PrivmsgMessage, permissions: &Permissions) -> bool {
        match permissions {
            Permissions::All => true,
            Permissions::Super => pm.sender.login == "boring_nick",
            Permissions::Mods => {
                pm.badges.iter().any(|badge| badge.name == "broadcaster")
                    | pm.badges.iter().any(|badge| badge.name == "moderator")
            }
            Permissions::Subs => {
                pm.badges.iter().any(|badge| badge.name == "broadcaster")
                    | pm.badges.iter().any(|badge| badge.name == "moderator")
                    | pm.badges.iter().any(|badge| badge.name == "subscriber")
            }
        }
    }

    fn parse_command(&self, msg: &str, channel: &str) -> Option<String> {
        let prefix = self
            .config
            .prefixes
            .get(channel)
            .expect("unconfigured for channel");
        if msg.starts_with(prefix) {
            Some(msg.strip_prefix(prefix).unwrap().to_string())
        } else {
            None
        }
    }

    async fn listen_msg(
        mut receiver: Receiver<SendMsg>,
        client: TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
    ) {
        println!("Starting message queue receiver");
        while let Some(msg) = receiver.recv().await {
            match msg {
                SendMsg::Say((channel, message)) => {
                    client.say(channel, message).await.expect("Failed to say")
                }
                SendMsg::Reply((message, reply_to)) => client
                    .reply_to_privmsg(message, &reply_to)
                    .await
                    .expect("Failed to reply"),
                SendMsg::Raw((channel, message)) => client
                    .privmsg(channel, message)
                    .await
                    .expect("Failed to privmsg"),
            }

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        println!("Error receiving message for sending!");
    }
}

#[derive(Debug)]
pub enum SendMsg {
    Reply((String, PrivmsgMessage)),
    Say((String, String)),
    Raw((String, String)),
}
