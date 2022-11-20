use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use askama::Template;
use uuid::Uuid;

#[derive(askama::Template)]
#[template(path = "home.html.j2")]
pub struct HomeTemplate {
    user_id: Option<Uuid>,
    flash_messages: Option<IncomingFlashMessages>,
}

pub async fn home() -> HttpResponse {
    let tpl = HomeTemplate {
        user_id: None,
        flash_messages: None,
    };

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tpl.render().unwrap())
}
