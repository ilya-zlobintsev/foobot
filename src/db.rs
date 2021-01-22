use mysql::{params, prelude::Queryable, Pool};
use std::{collections::HashMap, fmt};
use twitch_irc::login::StaticLoginCredentials;

use crate::{
    action_handler::Action,
    bot::BotConfig,
    command_handler::{Command, Permissions},
    config::DBConfig,
};

#[derive(Debug)]
pub enum DBConnError {
    AlreadyExists,
    NotFound,
    UnknownError(mysql::Error),
}

impl From<mysql::Error> for DBConnError {
    fn from(err: mysql::Error) -> Self {
        match &err {
            mysql::Error::MySqlError(sql_err) => match sql_err.code {
                1062 => DBConnError::AlreadyExists,
                _ => DBConnError::UnknownError(err),
            },
            _ => DBConnError::UnknownError(err),
        }
    }
}

impl fmt::Display for DBConnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DBConn error: {:?}", self)
    }
}

impl std::error::Error for DBConnError {}

#[derive(Debug, Clone)]
pub struct DBConn {
    pool: Pool,
}

impl DBConn {
    pub fn new(config: &DBConfig) -> Result<Self, mysql::Error> {
        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            config.user, config.password, config.host, config.port, config.db
        );

        let pool = Pool::new(url)?;

        Ok(DBConn { pool })
    }

    pub fn get_bot_config(&self) -> anyhow::Result<BotConfig> {
        let mut conn = self.pool.get_conn()?;

        let nickname: String = conn
            .query_first("SELECT value FROM settings WHERE option = \"nickname\"")?
            .unwrap();
        let oauth: String = conn
            .query_first("SELECT value FROM settings WHERE option = \"oauth\"")?
            .unwrap();

        let login = StaticLoginCredentials::new(
            nickname,
            Some(oauth.strip_prefix("oauth:").unwrap().to_owned()),
        );

        let channels: Vec<String> = conn.query_map("SELECT name FROM channels", |name| name)?;

        let prefixes = self.get_prefixes()?;

        Ok(BotConfig {
            login,
            oauth,
            prefixes,
            channels,
        })
    }

    pub fn get_command(
        &self,
        trigger: &str,
        channel: &str,
    ) -> Result<Option<Command>, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        match {
            conn.exec_map(
                "SELECT action, permissions FROM commands WHERE name = :trigger AND channel = :channel",
                params! {
                    trigger, channel,
                },
                |(action, permissions)| Command {
                    action: Action::Custom(action),
                    trigger: trigger.to_string(),
                    channel: channel.to_string(),
                    permissions: Permissions::from_string(permissions).unwrap(),
                },
            )?
            .first()
        } {
            Some(cmd) => Ok(Some(cmd.clone())),
            None => Ok(None),
        }
    }

    pub fn get_commands(&self, channel: &str) -> Result<Vec<String>, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        let mut commands = Vec::new();

        conn.exec_map(
            "SELECT name FROM commands WHERE channel = :channel",
            params! {
                "channel" => channel,
            },
            |name| commands.push(name),
        )?;

        Ok(commands)
    }

    pub fn add_command(
        &self,
        trigger: &str,
        action: &str,
        channel: &str,
        permissions: &str,
    ) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        conn.exec_drop(
            "INSERT INTO commands VALUES (:name, :action, :channel, :permissions)",
            params! {
                "name" => trigger,
                "action" => action,
                "channel" => channel,
                "permissions" => permissions,
            },
        )?;

        Ok(())
    }

    pub fn del_command(&self, trigger: &str, channel: &str) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        conn.exec_drop(
            "DELETE FROM commands WHERE channel = :channel AND name = :trigger",
            params! {
                "channel" => channel,
                "trigger" => trigger,
            },
        )?;

        match conn.affected_rows() {
            0 => Err(DBConnError::NotFound),
            1 => Ok(()),
            _ => unreachable!(),
        }
    }

    pub fn get_prefixes(&self) -> Result<HashMap<String, String>, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        let mut map = HashMap::new();

        conn.query_map("SELECT * FROM prefixes", |(channel, prefix)| {
            map.insert(channel, prefix)
        })?;

        Ok(map)
    }

    pub fn get_points_redeem_trigger(
        &self,
        name: &str,
        channel: &str,
    ) -> Result<Option<String>, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.exec_first(
            "SELECT action FROM redeem_triggers WHERE name = :name AND channel = :channel",
            params! {
                name, channel,
            },
        )?)
    }

    pub fn add_points_redeem_trigger(
        &self,
        name: &str,
        action: &str,
        channel: &str,
    ) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        conn.exec_drop(
            "INSERT INTO redeem_triggers VALUES (:channel, :name, :action)",
            params! {
                "channel" => channel,
                "name" => name,
                "action" => action,
            },
        )?;

        Ok(())
    }

    pub fn del_points_redeem_trigger(&self, name: &str, channel: &str) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        conn.exec_drop(
            "DELETE FROM redeem_triggers WHERE channel = :channel AND name = :name",
            params! {
                "channel" => channel,
                "name" => name,
            },
        )?;

        match conn.affected_rows() {
            0 => Err(DBConnError::NotFound),
            1 => Ok(()),
            _ => unreachable!(),
        }
    }

    pub fn add_hitman(&self, username: &str, channel: &str) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.exec_drop("INSERT INTO hitman VALUES (:channel, :name, :completed, :protected) ON DUPLICATE KEY UPDATE completed = :completed", 
                params! {
                    "channel" => channel,
                    "name" => username,
                    "completed" => false,
                    "protected" => false,
                })?)
    }

    pub fn get_hitman_protected(&self, username: &str, channel: &str) -> Result<bool, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        match conn.exec_first(
            "SELECT protected FROM hitman WHERE channel = :channel AND name = :name",
            params! {
                "channel" => channel,
                "name" => username,
            },
        )? {
            Some(result) => Ok(result),
            None => Ok(false),
        }
    }

    pub fn set_hitman_protection(
        &self,
        username: &str,
        channel: &str,
        protection: &bool,
    ) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.exec_drop("INSERT INTO hitman VALUES (:channel, :name, :completed, :protected) ON DUPLICATE KEY UPDATE protected = :protected", 
            params! {
                "channel" => channel,
                "name" => username,
                "completed" => false,
                "protected" => protection,
            })?)
    }

    ///Retruns a tuple of an access token and a refresh token.
    pub fn get_spotify_access_token(&self, channel: &str) -> Result<(String, String), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        let response = conn.exec(
            "SELECT access_token, refresh_token FROM spotify WHERE channel = :channel",
            params! {
                "channel" => channel,
            },
        )?;

        let (access_token, refresh_token): &(String, String) =
            response.first().ok_or_else(|| DBConnError::NotFound)?;

        Ok((access_token.clone(), refresh_token.clone()))
    }

    pub fn update_spotify_token(
        &self,
        channel: &str,
        access_token: &str,
    ) -> Result<(), DBConnError> {
        let mut conn = self.pool.get_conn()?;

        conn.exec_drop(
            "UPDATE spotify SET access_token = :access_token WHERE channel = :channel",
            params! {
                "access_token" => access_token,
                "channel" => channel,
            },
        )?;

        match conn.affected_rows() {
            0 => Err(DBConnError::NotFound),
            1 => Ok(()),
            _ => unreachable!(),
        }
    }

    pub fn get_spotify_refresh_tokens(&self) -> Result<Vec<(String, String)>, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.query("SELECT channel, refresh_token FROM spotify")?)
    }
    
    pub fn get_openweathermap_api_key(&self) -> Result<String, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.query_first("SELECT value FROM settings WHERE option = \"openweathermap\"")?.unwrap_or_default())
    }

    pub fn get_spotify_cilent_id(&self) -> Result<String, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.query_first("select value from settings where option = \"spotify_clientid\"")?.unwrap_or_default())
    }

    pub fn get_spotify_client_secret(&self) -> Result<String, DBConnError> {
        let mut conn = self.pool.get_conn()?;

        Ok(conn.query_first("select value from settings where option = \"spotify_clientsecret\"")?.unwrap_or_default())
    }

}
