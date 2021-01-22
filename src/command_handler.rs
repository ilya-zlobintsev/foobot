use anyhow::Result;
use twitch_irc::{login::StaticLoginCredentials, TCPTransport, TwitchIRCClient};

use crate::{
    action_handler::{Action, ActionHandler},
    db::{DBConn, DBConnError},
    twitch_api::TwitchApi,
};

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
        client: TwitchIRCClient<TCPTransport, StaticLoginCredentials>,
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
                        }
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
                'outer: while let Some(ch) = chars.next() {
                    match ch {
                        '{' => {
                            let mut action = String::new();
                            let mut action_args: Vec<String> = Vec::new();

                            while let Some(ch) = chars.next() {
                                match ch {
                                    '}' => {
                                        match self
                                            .action_handler
                                            .run(&action, &action_args, channel, &client)
                                            .await
                                        {
                                            Ok(action_result) => {
                                                //Doesn't respond with anything the action doesn't yield a result
                                                if let Some(result) = action_result {
                                                    response.push_str(&result);
                                                }
                                                continue 'outer;
                                            }
                                            Err(e) => return Err(e),
                                        }
                                    }
                                    '$' => {
                                        if let Some(ch) = chars.next() {
                                            println!("Getting variable {}", ch.to_string());
                                            // Numeric variables represent command arguments e.g. `!command 1` where the command is `{action $0}` will execute `action` with the argument `1`.
                                            if let Some(num) = ch.to_digit(10) {
                                                if let Some(arg) = args.get(num as usize) {
                                                    action_args.push((*arg).to_owned());
                                                } else {
                                                    return Err(
                                                        CommandHandlerError::ExecutionError(
                                                            String::from("missing argument"),
                                                        ),
                                                    );
                                                }
                                            // `$$` will pass all command arguments to the action as a single argument.
                                            } else if ch == "$".chars().next().unwrap() {
                                                action_args.push(args.join(" "));
                                            } else {
                                                return Err(CommandHandlerError::ExecutionError(
                                                    String::from("invalid variable"),
                                                ));
                                            }
                                        } else {
                                            return Err(CommandHandlerError::ExecutionError(
                                                String::from("missing variable"),
                                            ));
                                        }
                                    }
                                    ' ' => continue,
                                    _ => action.push(ch),
                                }
                            }
                            return Err(CommandHandlerError::ExecutionError(String::from(
                                "closing } not found",
                            )));
                        }
                        _ => response.push(ch),
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
