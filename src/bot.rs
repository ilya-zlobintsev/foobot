use std::{collections::HashMap, time::Duration};

use tokio::{task, time::sleep};
use twitch_irc::{ClientConfig, TCPTransport, TwitchIRCClient, login::StaticLoginCredentials, message::{PrivmsgMessage, ServerMessage}};

use crate::{command_handler::{CommandHandler, CommandHandlerError, Permissions}, db::DBConn, jobs::JobRunner, pubsub::PubSubHandler, twitch_api::TwitchApi};

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
        let pubsub_handler = PubSubHandler::new(TwitchApi::init(&config.oauth).await?, db_conn.clone());
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
        
        let loop_client = client.clone();

        let join_handle = task::spawn(async move {
            while let Some(message) = incoming_messages.recv().await {
                match message {
                    ServerMessage::Privmsg(privmsg) => {
                        // println!("#{} {}: {}", privmsg.channel_login, privmsg.sender.name, privmsg.message_text);
                        self.handle_privmsg(privmsg, loop_client.clone())
                    },
                    _ => continue,
                }
            }
        });

        for channel in &channels {
            println!("Joining {}", channel);
            client.join(channel.to_owned());
            client.say(channel.to_owned(), "Bot started!".to_owned()).await.unwrap();
        }

        // let quit_handle = runner.quit_handle();
        
        job_runner.start().await?;

        task::spawn(async move {
            loop {
                pubsub_handler.start(&channels, &client).await;
                println!("Pubsub: reconnecting...");
                sleep(Duration::from_secs(5)).await;
            }
        });
        
        join_handle.await?;

        Ok(())
    }

    fn handle_privmsg(&self, pm: PrivmsgMessage, client: TwitchIRCClient<TCPTransport, StaticLoginCredentials>) {
        if let Some(cmd) = self.parse_command(&pm.message_text, &pm.channel_login) {
            println!("{} {}: {}", pm.channel_login, pm.sender.login, pm.message_text);
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

                                match command_handler.run_command(&cmd, arguments, &pm.channel_login, &pm.sender.login, client.clone()).await {
                                    Ok(execution) => {
                                        if let Some(response) = execution {
                                            println!("Responding with {}", response);

                                            if let Err(_) = client.reply_to_privmsg(response, &pm).await { println!("Error replying"); }
                                        } else {
                                            println!("Execution finished (no response)");
                                        }
                                    }
                                    Err(err) => {
                                        match err {
                                            CommandHandlerError::ExecutionError(err) => {
                                                if let Err(_) = client.reply_to_privmsg(format!("execution error: {}", err), &pm).await { println!("Error replying"); }
                                            }
                                            CommandHandlerError::DBError(err) => {
                                                if let Err(_) = client.reply_to_privmsg(format!("DB error: {}", err), &pm).await { println!("Error replying"); }
                                            }
                                            _ => unreachable!(),
                                        }
                                    }
                                }
                            } 
                            else {
                                if let Err(_) = client.say(pm.channel_login, format!("@{}, you do not have the permissions to use this command", pm.sender.login)).await { println!("Failed responding about incorrect permissions") }
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
            Permissions::Super => pm.badges.iter().any(|badge| badge.name == "broadcaster") | (pm.sender.login == "boring_nick"),
            Permissions::Mods => pm.badges.iter().any(|badge| badge.name == "broadcaster") | pm.badges.iter().any(|badge| badge.name == "moderator"),
            Permissions::Subs => pm.badges.iter().any(|badge| badge.name == "broadcaster") | pm.badges.iter().any(|badge| badge.name == "moderator") | pm.badges.iter().any(|badge| badge.name == "subscriber"),
        }
    }

    fn parse_command(&self, msg: &str, channel: &str) -> Option<String> {
        let prefix = self.config.prefixes.get(channel).expect("unconfigured for channel");
        if msg.starts_with(prefix) {
            Some(msg.strip_prefix(prefix).unwrap().to_string())
        } else {
            None
        }
    }
}
