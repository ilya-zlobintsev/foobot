#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate rocket;

mod action_handler;
mod bot;
mod command_handler;
mod config;
mod db;
mod jobs;
mod pubsub;
mod twitch_api;
mod web;

use bot::Bot;
use config::DBConfig;
use db::DBConn;
use tokio::{fs, task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_config_json = fs::read_to_string("config.json").await?;
    let db_config = DBConfig::from_json(&db_config_json)?;

    let db_conn = DBConn::new(&db_config)?;

    {
        let db_conn = db_conn.clone();
        task::spawn(async move {
            web::run(db_conn);
        });
    }

    let bot = Bot::new(db_conn).await?;
    bot.run().await?;

    Ok(())
}
