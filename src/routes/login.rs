use actix_web::HttpResponse;
use actix_web::http::header::ContentType;
use askama::Template;

#[derive(askama::Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {}

pub async fn login() -> HttpResponse {
    let tpl = LoginTemplate {};

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}
