use std::fmt::{Debug, Display};
use tracing::{error, info};
use zero2prod::configuration::get_configuration;
use zero2prod::issue_delivery_worker;
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

    let app = Application::build(configuration.clone()).await?;

    let app_task = tokio::spawn(app.run_until_stopped());
    let worker_task = tokio::spawn(issue_delivery_worker::run_worker_until_stopped(
        configuration,
    ));

    tokio::select! {
        outcome = app_task => report_exit("API", outcome),
        outcome = worker_task => report_exit("Issue delivery worker", outcome),
    };

    Ok(())
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl Debug + Display>, tokio::task::JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            info!("{} has exited", task_name);
        }
        Ok(Err(err)) => {
            error!(
                error.cause_chain = ?err,
                error.message = %err,
                "{} failed",
                task_name,
            )
        }
        Err(err) => {
            error!(
                error.cause_chain = ?err,
                error.message = %err,
                "'{}' task failed to complete",
                task_name,
            )
        }
    }
}
