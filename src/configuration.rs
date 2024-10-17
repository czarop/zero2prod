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
    pub application_port: u16,
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
    // initialise the config reader
    let settings = config::Config::builder()
        // add config values fromt he file, configuratrion.yaml
        .add_source(config::File::new(
            "configuration.yaml",
            config::FileFormat::Yaml,
        ))
        .build()?;

    // try to convert the config values into
    // our Settings struct
    settings.try_deserialize::<Settings>()
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
