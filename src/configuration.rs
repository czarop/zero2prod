use secrecy::ExposeSecret;
use secrecy::Secret;
use serde_aux::field_attributes::deserialize_number_from_string;
// instead of a connection string - this structure holds the options for db connection
use crate::domain::SubscriberEmail;
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgSslMode; // for secure db connection

// this code reads in and outputs app-specific settings from
// and to a file, configuration.yaml

// A struct holding settings relevent to this run
#[derive(serde::Deserialize)]
pub struct Settings {
    // settings for the db
    pub database: DatabaseSettings,
    // the port on which the app is listening for db updates
    pub application: ApplicationSettings,

    pub email_client: EmailClientSettings,
}

// port listening on and host environemnt (docker image - production, or debug)
#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    // this allows deserialisation of numbers - needs the serde_aux dependency and import
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
}

// A struct holding settings relevent to setting up the db
// this has to impl Deserialize so it can be used in above Struct
#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>, // this will be redacted unless unwrapped
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    // determine if we need secure connection
    pub require_ssl: bool,
}

// generate a connection_string from data in the config struct, which will allow us to connect
// to the database with PgConnect
impl DatabaseSettings {
    pub fn connection_options(&self) -> PgConnectOptions {
        // if we are local, don't need ssl, if we are production, we do
        // this is specified in the YAML config files
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        // when in production, these come from spec.yaml - which dynamically
        // gets these from digitalocean db instance.
        // when local, this is coming from base.yaml
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
            .database(&self.database_name)
    }
}

// data structure to hold info about the email 'sender' - ie postmark and your email address
// these will be grabbed from config/production or config/base on startup
#[derive(serde::Deserialize)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub auth_token: Secret<String>,
    pub timeout_milliseconds: u64,
}

impl EmailClientSettings {
    /// Returns the sender_email of this [`EmailClientSettings`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the address is not valid.
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_milliseconds)
    }
}

// we will read our configuration settings from a file configuration.yaml
pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");

    let configuration_directory = base_path.join("configuration");
    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let environment_filename = format!("{}.yaml", environment.as_str());

    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        // Add in settings from environment variables (with a prefix of APP and
        // '__' as separator)
        // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}
/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}
impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. \
                Use either `local` or `production`.",
                other
            )),
        }
    }
}
