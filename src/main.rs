use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use zero2prod::telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    //

    let configuration = get_configuration().expect("Failed to read configuration");

    tracing::info!(
        application_host = %configuration.application.host,
        application_port = %configuration.application.port,
        "got configuration",
    );

    let app = Application::build(configuration).await?;
    app.run_until_stopped().await?;

    Ok(())
}
