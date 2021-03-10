use rocket::State;
use rocket_contrib::templates::Template;

use crate::{command_handler::Command, db::DBConn};

#[derive(serde::Serialize)]
struct CommandsPageContext {
    channel: String,
    commands: Vec<Command>,
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/commands/<channel>")]
fn get_commands(db_conn: State<DBConn>, channel: String) -> Template {
    let mut commands: Vec<Command> = Vec::new();

    for command in db_conn.get_commands(&channel).unwrap() {
        commands.push(db_conn.get_command(&command, &channel).unwrap().unwrap());
    }

    Template::render("commands-page", &CommandsPageContext { channel, commands })
}

pub fn run(db_conn: DBConn) {
    println!("Initializing web");

    rocket::ignite()
        .manage(db_conn)
        .mount("/", routes![index, get_commands])
        .attach(Template::fairing())
        .launch();
}
