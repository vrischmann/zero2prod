use actix_web::HttpResponse;
use actix_web::http::header::ContentType;
use askama::Template;

#[derive(askama::Template)]
#[template(path = "home.html.j2")]
pub struct HomeTemplate {}

pub async fn home() -> HttpResponse {
    let tpl = HomeTemplate {};

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}
