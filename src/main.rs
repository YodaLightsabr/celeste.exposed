// mod copilot;
mod notify;
mod poll;

#[macro_use]
extern crate rocket;
use rocket::{
    fs::NamedFile,
    http::Status,
    response::status,
    serde::json::Json,
    shield::{Frame, Shield},
};
use rocket_dyn_templates::Template;

use comrak::{markdown_to_html, ComrakExtensionOptions, ComrakOptions, ComrakRenderOptions};
use dotenv::dotenv;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

// pages

#[derive(Serialize)]
struct Page {
    title: String,
    body: String,
}

fn render(title: &str, path: &str) -> Result<Template, Status> {
    let body = fs::read_to_string(path).map_err(|_| Status::NotFound)?;
    let options = ComrakOptions {
        extension: ComrakExtensionOptions {
            footnotes: true,
            ..ComrakExtensionOptions::default()
        },
        render: ComrakRenderOptions {
            unsafe_: true,
            ..ComrakRenderOptions::default()
        },
        ..ComrakOptions::default()
    };
    Ok(Template::render(
        "page",
        &Page {
            title: title.to_string(),
            body: markdown_to_html(&body, &options).to_string(),
        },
    ))
}

#[catch(404)]
async fn on_404() -> Result<Template, Status> {
    render("404", "pages/404.md")
}

#[get("/")]
async fn index_page() -> Result<Template, Status> {
    render("index", "pages/index.md")
}

#[get("/<file>")]
async fn other_file(file: String) -> Result<NamedFile, Result<Template, Status>> {
    match NamedFile::open(Path::new("static/").join(PathBuf::from(&file)))
        .await
        .ok()
    {
        Some(file) => Ok(file),
        None => Err(render(
            file.clone().as_str(),
            format!("pages/{}.md", file).as_str(),
        )),
    }
}

// api

fn json_error(status: Status, message: String) -> status::Custom<Json<Value>> {
    status::Custom(status, Json(json!({ "error": message })))
}

// #[post("/api/copilot", format = "json", data = "<data>")]
// fn copilot_endpoint(
//     data: Json<copilot::Request>,
// ) -> Result<Json<Value>, status::Custom<Json<Value>>> {
//     let copilot::Request {
//         prompt,
//         max_tokens,
//         temperature,
//         top_p,
//     } = data.into_inner();
//     let temperature = temperature.unwrap_or(1.0);
//     let top_p = top_p.unwrap_or(0.9);
//
//     match notify::notify(
//         format!("copilot request with prompt: {}", prompt.clone()).as_str(),
//         "airplane",
//     ) {
//         Ok(_) => (),
//         Err(e) => return Err(json_error(Status::InternalServerError, e.to_string())),
//     }
//
//     let output = copilot::get_copilot(prompt, max_tokens, temperature, top_p);
//     match output {
//         Ok(output) => Ok(Json(json!({ "ok": true, "output": output }))),
//         Err(_) => Err(json_error(Status::BadRequest, "...".to_string())),
//     }
// }

#[derive(serde::Deserialize)]
struct Feedback {
    feedback: String,
}

#[post("/api/feedback", format = "json", data = "<feedback>")]
fn feedback_endpoint(feedback: Json<Feedback>) {
    let Feedback { feedback } = feedback.into_inner();
    notify::notify(
        format!("NEW FEEDBACK: {}", feedback).as_str(),
        "love_letter",
    )
    .unwrap()
}

#[get("/api/poll/get/<poll_id>")]
fn get_poll_endpoint(poll_id: String) -> Result<Json<Value>, status::Custom<Json<Value>>> {
    match poll::get_poll(poll_id) {
        Ok(Some(poll)) => Ok(Json(json!({ "ok": true, "poll": poll }))),
        Ok(None) => Err(json_error(
            Status::NotFound,
            "this poll doesn't exist".to_string(),
        )),
        Err(_) => Err(json_error(
            Status::InternalServerError,
            "unknown error".to_string(),
        )),
    }
}

#[post("/api/poll/create", format = "json", data = "<options>")]
fn new_poll_endpoint(options: Json<Vec<String>>) -> Json<Value> {
    let poll_id = poll::create_poll(options.into_inner());
    notify::notify(
        format!("NEW POLL: https://celeste.exposed/poll/{}", poll_id).as_str(),
        "ballot_box",
    )
    .unwrap();
    Json(json!({
        "ok": true,
        "url": format!("https://celeste.exposed/poll/{}", poll_id)
    }))
}

#[post("/api/poll/vote", format = "json", data = "<data>")]
fn vote_poll_endpoint(data: Json<poll::PollVote>) -> Json<Value> {
    let poll::PollVote {
        poll_id,
        option,
        fingerprint,
    } = data.into_inner();
    match poll::vote_poll(poll_id, option, fingerprint) {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[post("/api/poll/voted", format = "json", data = "<data>")]
fn voted_poll_endpoint(data: Json<poll::PollVoteCheck>) -> Json<Value> {
    let poll::PollVoteCheck {
        poll_id,
        fingerprint,
    } = data.into_inner();
    match poll::voted_on(fingerprint, poll_id) {
        Ok(voted) => Json(json!({ "ok": true, "voted": voted })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[get("/poll/<poll_id>")]
fn poll_page(poll_id: String) -> Result<Template, Status> {
    let res = poll::get_poll(poll_id.clone());
    match res {
        Ok(Some(data)) => Ok(Template::render("poll", poll::Poll { id: poll_id, data })),
        Ok(None) => Err(Status::NotFound),
        Err(_) => Err(Status::InternalServerError),
    }
}

#[derive(serde::Deserialize)]
struct PageVisit {
    url: String,
}

#[post("/api/visited", format = "json", data = "<data>")]
fn visited_endpoint(data: Json<PageVisit>, address: SocketAddr) {
    let PageVisit { url } = data.into_inner();
    notify::notify(format!("VISIT: {} from {}", url, address).as_str(), "eye").unwrap();
}

// main

#[launch]
fn rocket() -> _ {
    dotenv().ok();
    rocket::build()
        .mount(
            "/",
            routes![
                index_page,
                other_file,
                // copilot_endpoint,
                feedback_endpoint,
                get_poll_endpoint,
                new_poll_endpoint,
                vote_poll_endpoint,
                voted_poll_endpoint,
                poll_page,
                visited_endpoint,
            ],
        )
        .register("/", catchers![on_404])
        .attach(Template::fairing())
        .attach(Shield::default().disable::<Frame>())
}
