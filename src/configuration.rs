use secrecy::ExposeSecret;
use secrecy::Secret;

// this code reads in and outputs app-specific settings from
// and to a file, configuration.yaml

// A struct holding settings relevent to this run
#[derive(serde::Deserialize)]
pub struct Settings {
    // settings for the db
    pub database: DatabaseSettings,
    // the port on which the app is listening for db updates
    pub application: ApplicationSettings,
}

// port listening on and host environemnt (docker image - production, or debug)
#[derive(serde::Deserialize)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
}

// A struct holding settings relevent to setting up the db
// this has to impl Deserialize so it can be used in above Struct
#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>, // this will be redacted unless unwrapped
    pub port: u16,
    pub host: String,
    pub database_name: String,
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

// generate a connection_string from data in the config struct, which will allow us to connect
// to the database with PgConnect
impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        // we the connection string a secret
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(), // exposed as redacted above
            self.host,
            self.port,
            self.database_name
        ))
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
