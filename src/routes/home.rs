use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use askama::Template;

#[derive(askama::Template)]
#[template(path = "home.html.j2")]
pub struct HomeTemplate {
    error_messages: Vec<String>,
    info_messages: Vec<String>,
}

pub async fn home() -> HttpResponse {
    let tpl = HomeTemplate {
        error_messages: Vec::new(),
        info_messages: Vec::new(),
    };

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}
