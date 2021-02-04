#[macro_use]
extern crate serde;

mod bot;
mod config;
mod db;
mod command_handler;
mod action_handler;
mod jobs;
mod pubsub;
mod twitch_api;

use bot::Bot;
use config::DBConfig;
use db::DBConn;
use tokio::fs;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_config_json = fs::read_to_string("config.json").await?;
    let db_config = DBConfig::from_json(&db_config_json)?;
    
    let db_conn = DBConn::new(&db_config)?;
    
    let bot = Bot::new(db_conn).await?;
    bot.run().await?;
        
    Ok(())
}