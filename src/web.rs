use crate::twitch_api::TwitchApi;
use reqwest::{Client, Url};
use rocket::{response::Redirect, State};
use rocket_dyn_templates::Template;
use serde_json::Value;

use crate::{command_handler::Command, db::DBConn};

const CLIENT_ID: &'static str = "pl0ptknnjoq305qfrw0slqpl0pux33";
const CLIENT_SECRET: &'static str = "we2b03bv89c93jf2lw7xiyyp8pq6gi";
const REDIRECT_URI: &'static str = "https://bot.endpoint.ml/auth/callback";

#[derive(serde::Serialize)]
struct CommandsPageContext {
    channel: String,
    commands: Vec<Command>,
}

#[get("/")]
fn index() -> &'static str {
    "go to /commands/{channel}"
}

#[get("/commands/<channel>")]
fn get_commands(db_conn: &State<DBConn>, channel: String) -> Template {
    let mut commands: Vec<Command> = Vec::new();

    for command in db_conn.get_commands(&channel).unwrap() {
        commands.push(db_conn.get_command(&command, &channel).unwrap().unwrap());
    }

    Template::render("commands-page", &CommandsPageContext { channel, commands })
}

#[get("/auth")]
fn auth() -> Redirect {
    let url = Url::parse_with_params(
        "https://id.twitch.tv/oauth2/authorize",
        &[
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("response_type", "code"),
            ("scope", ""),
        ],
    )
    .unwrap();

    Redirect::to(url.to_string())
}

#[get("/auth/callback?<code>&<scope>")]
async fn auth_callback(code: String, scope: String) -> String {
    let client = Client::new();

    let access_token = client
        .post("https://id.twitch.tv/oauth2/token")
        .query(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", &code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", REDIRECT_URI),
        ])
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["access_token"]
        .to_string()
        .replace("\"", "");

    let twitch_api = TwitchApi::init(&access_token)
        .await
        .expect("Failed to build twitch API");

    let users_response = twitch_api.get_users_by_login(&Vec::new()).await.unwrap();

    let user = users_response.data.first().unwrap();

    format!("Hello {}", user.display_name.replace("\"", ""))
}

pub async fn run(db_conn: DBConn) {
    println!("Initializing web");

    rocket::build()
        .manage(db_conn)
        .mount("/", routes![index, get_commands, auth, auth_callback])
        .attach(Template::fairing())
        .launch()
        .await
        .unwrap();
}
