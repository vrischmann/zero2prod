use actix_web::http::header::{ContentType, LOCATION};
use actix_web::web;
use actix_web::HttpResponse;
use askama::Template;
use secrecy::Secret;

#[derive(askama::Template)]
#[template(path = "login.html.j2")]
pub struct LoginTemplate {}

pub async fn login() -> HttpResponse {
    let tpl = LoginTemplate {};

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

pub async fn do_login(form: web::Form<LoginFormData>) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish()
}
