use zero2prod::configuration;
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
    let application = Application::build(configuration).await?;
    application.run_until_stopped().await?;
    Ok(())
}
