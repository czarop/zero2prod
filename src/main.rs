use std::fmt::{Debug, Display};
use tokio::task::JoinError;
use zero2prod::configuration;
use zero2prod::issue_delivery_worker;
use zero2prod::startup::Application;
use zero2prod::telemetry;

#[tokio::main] // a procedural macro that wraps synchronous main() in async fn -
               // otherwise async main not allowed, and this return type not allowed
async fn main() -> anyhow::Result<()> {
    // set up trace and logging
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);

    telemetry::init_subscriber(subscriber);

    // Panic if we can't read the config file
    let configuration =
        configuration::get_configuration().expect("Failed to read configuration.yaml");

    // await the future here - we can call main as a non-blocking
    // task in tests etc
    let application = Application::build(configuration.clone()).await?; // build the app

    // the tokio::spawn will run each task in a separate thread
    let application_task = tokio::spawn(application.run_until_stopped());

    // start a concurrent task to look for new 'newsletter to send' entries in the email to send table
    let worker_task = tokio::spawn(issue_delivery_worker::run_worker_until_stopped(
        configuration,
    ));

    // select the tasks to run and run them
    tokio::select! {
        o = application_task => report_exit("API", o), // this will be called when the task completes
        o = worker_task => report_exit("Background worker", o),
    };

    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}
