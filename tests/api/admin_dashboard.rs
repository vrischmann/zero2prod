use crate::helpers::{assert_is_redirect_to, spawn_app, LoginBody};

#[tokio::test]
async fn must_be_logged_in_to_access_the_admin_dashboard() {
    let app = spawn_app().await;

    let response = app.get_admin_dashboard().await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn logout_clears_session_state() {
    let app = spawn_app().await;

    // 1) login
    let response = app
        .post_login(&LoginBody {
            username: app.test_user.username.clone(),
            password: app.test_user.password.clone(),
        })
        .await;
    assert_is_redirect_to(&response, "/admin/dashboard");

    let html_page = app.get_admin_dashboard_html().await;
    assert!(html_page.contains(&format!("Welcome {}", &app.test_user.username)));

    // 2) logout and fetch the login form to check the flash messages
    let response = app.post_logout().await;
    assert_is_redirect_to(&response, "/login");

    let html_page = app.get_login_html().await;
    assert!(html_page.contains("You have successfully logged out"));

    // 3) reload the admin dashboard but this time expect to be redirect
    let response = app.get_admin_dashboard().await;
    assert_is_redirect_to(&response, "/login");
}
