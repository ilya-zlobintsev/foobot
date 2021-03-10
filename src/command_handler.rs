use anyhow::Result;
use tokio::sync::mpsc::Sender;

use crate::{action_handler::{Action, ActionHandler}, bot::SendMsg, db::{DBConn, DBConnError}, twitch_api::TwitchApi};

#[derive(Clone, Debug)]
pub enum Permissions {
    All,
    Subs,
    Mods,
    Super,
}

impl Permissions {
    pub fn from_string(s: String) -> Result<Self, CommandHandlerError> {
        match s.as_str() {
            "all" => Ok(Permissions::All),
            "subs" => Ok(Permissions::Subs),
            "mods" => Ok(Permissions::Mods),
            "super" => Ok(Permissions::Super),
            _ => Err(CommandHandlerError::InvalidPermissions),
        }
    }

    // pub fn to_string(&self) -> String {
    //     match self {
    //         Permissions::All => String::from("all"),
    //         Permissions::Subs => String::from("subs"),
    //         Permissions::Mods => String::from("mods"),
    //         Permissions::Super => String::from("super"),
    //     }
    // }
}

#[derive(Debug)]
pub enum CommandHandlerError {
    DBError(DBConnError),
    ExecutionError(String),
    WriterError(std::io::Error),
    ReqwestError(reqwest::Error),
    IRCError(String),
    InvalidPermissions,
}

impl From<DBConnError> for CommandHandlerError {
    fn from(db_error: DBConnError) -> Self {
        CommandHandlerError::DBError(db_error)
    }
}

impl From<std::io::Error> for CommandHandlerError {
    fn from(io_error: std::io::Error) -> Self {
        CommandHandlerError::WriterError(io_error)
    }
}

impl From<reqwest::Error> for CommandHandlerError {
    fn from(reqwest_error: reqwest::Error) -> Self {
        CommandHandlerError::ReqwestError(reqwest_error)
    }
}

#[derive(Clone, Debug)]
pub struct Command {
    pub trigger: String,
    pub action: Action,
    pub channel: String,
    pub permissions: Permissions,
}

#[derive(Clone)]
pub struct CommandHandler {
    db_conn: DBConn,
    action_handler: ActionHandler,
}

impl CommandHandler {
    pub fn new(db_conn: DBConn, twitch_api: TwitchApi) -> Self {
        let action_handler = ActionHandler::new(db_conn.clone(), twitch_api);
        Self {
            db_conn,
            action_handler,
        }
    }

    pub fn get_command(
        &self,
        trigger: &str,
        channel: &str,
    ) -> Result<Option<Command>, CommandHandlerError> {
        match trigger {
            "ping" => Ok(Some(Command {
                trigger: String::from("ping"),
                action: Action::Custom(String::from("{ping}")),
                channel: String::from(channel),
                permissions: Permissions::All,
            })),
            "addcmd" => Ok(Some(Command {
                trigger: String::from("addcmd"),
                action: Action::AddCmd,
                channel: String::from(channel),
                permissions: Permissions::Mods,
            })),
            "showcmd" => Ok(Some(Command {
                trigger: String::from("showcmd"),
                action: Action::ShowCmd,
                channel: String::from(channel),
                permissions: Permissions::All,
            })),
            "help" | "commands" => Ok(Some(Command {
                trigger: String::from("commands"),
                action: Action::ListCmd,
                channel: String::from(channel),
                permissions: Permissions::All,
            })),
            "delcmd" => Ok(Some(Command {
                trigger: String::from("delcmd"),
                action: Action::DelCmd,
                channel: String::from(channel),
                permissions: Permissions::Mods,
            })),
            "join" => Ok(Some(Command {
                trigger: String::from("join"),
                action: Action::Join,
                channel: String::from(channel),
                permissions: Permissions::Super,
            })),
            "part" => Ok(Some(Command {
                trigger: String::from("part"),
                action: Action::Part,
                channel: String::from(channel),
                permissions: Permissions::Super,
            })),
            _ => Ok(self.db_conn.get_command(trigger, channel)?),
        }
    }

    pub async fn run_command(
        &self,
        cmd: &Command,
        args: &[&str],
        channel: &str,
        runner: &str,
        msg_sender: Sender<SendMsg>
    ) -> Result<Option<String>, CommandHandlerError> {
        let mut response = String::new();

        match &cmd.action {
            Action::AddCmd => {
                if let Some((trigger, action)) = args.split_first() {
                    let action = action.join(" ");

                    match self.db_conn.add_command(trigger, &action, channel, "all") {
                        Ok(()) => Ok(Some(format!("successfully added command \"{}\"", trigger))),
                        Err(err) => match err {
                            DBConnError::AlreadyExists => {
                                Ok(Some(format!("command \"{}\" already exists", trigger)))
                            }
                            _ => Err(CommandHandlerError::DBError(err)),
                        },
                    }
                } else {
                    Ok(Some(String::from("missing arguments")))
                }
            }
            Action::DelCmd => {
                if let Some(trigger) = args.first() {
                    match self.db_conn.del_command(trigger, channel) {
                        Ok(()) => Ok(Some(format!(
                            "successfully removed command \"{}\"",
                            trigger
                        ))),
                        Err(err) => match err {
                            DBConnError::NotFound => {
                                Ok(Some(format!("command \"{}\" not found", trigger)))
                            }
                            _ => Err(CommandHandlerError::DBError(err)),
                        },
                    }
                } else {
                    Ok(Some(String::from("missing command")))
                }
            }
            Action::ShowCmd => {
                if let Some(trigger) = args.first() {
                    match self.db_conn.get_command(trigger, channel)? {
                        Some(cmd) => match cmd.action {
                            Action::Custom(action) => Ok(Some(format!("{}", action))),
                            _ => Ok(Some(String::from("built-in command"))),
                        },
                        None => Ok(Some(String::from("no such command"))),
                    }
                } else {
                    Ok(Some(String::from("command not specified")))
                }
            }
            Action::ListCmd => {
                let commands = self.db_conn.get_commands(channel)?;
                Ok(Some(format!("Custom commands: {:?}", commands)))
            }
            Action::Join => Ok(None),
            Action::Part => Ok(None),
            Action::Custom(cmd) => {
                let mut chars = cmd.chars();

                while let Some(ch) = chars.next() {
                    if ch == '{' {
                        let mut action = String::new();

                        while let Some(ch) = chars.next() {
                            if ch == '}' {
                                break;
                            }
                            action.push(ch);
                        }

                        let split: Vec<&str> = action.split_whitespace().collect();
                        let (action, raw_args) = split.split_first().unwrap();

                        let mut action_args: Vec<String> = Vec::new();

                        //Parses variables in action arguments
                        for arg in raw_args {
                            if let Some(var) = arg.strip_prefix("$") {
                                match var {
                                    //$$: Pass all command arguments
                                    "$" => {
                                        for arg in args {
                                            action_args.push(arg.to_string());
                                        }
                                    }
                                    _ => {
                                        if var == "user" {
                                            action_args.push(runner.to_string());
                                        }
                                        //Arguments that take positional arguments of the command, such as $0
                                        else if let Some(num) =
                                            var.chars().next().unwrap().to_digit(10)
                                        {
                                            match args.get(num as usize) {
                                                Some(arg) => action_args.push(arg.to_string()),
                                                None => {
                                                    return Err(
                                                        CommandHandlerError::ExecutionError(
                                                            format!(
                                                                "missing argument index {}",
                                                                num
                                                            ),
                                                        ),
                                                    )
                                                }
                                            }
                                        } else {
                                            return Err(CommandHandlerError::ExecutionError(
                                                "invalid variable".to_string(),
                                            ));
                                        }
                                    }
                                }
                            } else {
                                action_args.push(arg.to_string());
                            }
                        }

                        match self
                            .action_handler
                            .run(action, &action_args, channel, msg_sender.clone())
                            .await
                        {
                            Ok(Some(action_response)) => response.push_str(&action_response),
                            //Don't respond with anything if the action doesn't produce anything
                            Ok(None) => (),
                            Err(e) => return Err(e),
                        }
                    } else {
                        response.push(ch);
                    }
                }

                if !response.is_empty() {
                    Ok(Some(response))
                } else {
                    Ok(None)
                }
            }
        }
    }
}
