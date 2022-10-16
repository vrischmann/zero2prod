use actix_web::{web, HttpResponse, Responder};

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

pub async fn subscribe(form: web::Form<FormData>) -> impl Responder {
    let inner = form.into_inner();
    let _ = inner.name;
    let _ = inner.email;

    HttpResponse::Ok()
}
