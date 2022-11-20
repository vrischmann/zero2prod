use crate::helpers::{assert_is_redirect_to, spawn_app};
use crate::helpers::{AdminChangePasswordBody, LoginBody};
use uuid::Uuid;

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_change_password_form() {
    let app = spawn_app().await;

    let response = app.get_admin_change_password().await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    let app = spawn_app().await;

    let new_password = Uuid::new_v4().to_string();

    let response = app
        .post_admin_change_password(&AdminChangePasswordBody {
            current_password: Uuid::new_v4().to_string(),
            new_password: new_password.clone(),
            new_password_check: new_password,
        })
        .await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn new_password_fields_must_match() {
    let app = spawn_app().await;

    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    // Login
    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Try to change the password
    let response = app
        .post_admin_change_password(&AdminChangePasswordBody {
            current_password: app.test_user.password.clone(),
            new_password: new_password.clone(),
            new_password_check: another_new_password.clone(),
        })
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_admin_change_password_html().await;
    assert!(html_page.contains("You entered two different new passwords"));
}

#[tokio::test]
async fn current_password_is_invalid() {
    let app = spawn_app().await;

    let new_password = Uuid::new_v4().to_string();
    let wrong_password = Uuid::new_v4().to_string();

    // Login
    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Try to change the password
    let response = app
        .post_admin_change_password(&AdminChangePasswordBody {
            current_password: wrong_password.clone(),
            new_password: new_password.clone(),
            new_password_check: new_password.clone(),
        })
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_admin_change_password_html().await;
    assert!(html_page.contains("The current password is incorrect"));
}

#[tokio::test]
async fn new_password_is_too_short() {
    let app = spawn_app().await;

    let new_password = "foo".to_string();

    // Login
    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Try to change the password
    let response = app
        .post_admin_change_password(&AdminChangePasswordBody {
            current_password: app.test_user.password.clone(),
            new_password: new_password.clone(),
            new_password_check: new_password.clone(),
        })
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_admin_change_password_html().await;

    assert!(html_page.contains("New password is too short"));
}

#[tokio::test]
async fn new_password_is_too_long() {
    let app = spawn_app().await;

    let new_password = "a".repeat(200);

    // Login
    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Try to change the password
    let response = app
        .post_admin_change_password(&AdminChangePasswordBody {
            current_password: app.test_user.password.clone(),
            new_password: new_password.clone(),
            new_password_check: new_password.clone(),
        })
        .await;
    assert_is_redirect_to(&response, "/admin/password");

    // Follow the redirect
    let html_page = app.get_admin_change_password_html().await;
    assert!(html_page.contains("New password is too long"));
}
