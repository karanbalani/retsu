mod defaults;
mod error;
mod schema;

use std::path::Path;
use validator::Validate;

pub(crate) use error::ConfigurationError;
pub(crate) use schema::{AppConfiguration, DatabaseConfig, LogFormat};

pub(crate) fn load(config_path: Option<&Path>) -> Result<AppConfiguration, ConfigurationError> {
    load_with_environment(config_path, environment_source())
}

fn load_with_environment(
    config_path: Option<&Path>,
    environment: config::Environment,
) -> Result<AppConfiguration, ConfigurationError> {
    let builder = config::Config::builder();

    let builder = match config_path {
        Some(path) => builder.add_source(config::File::from(path).required(true)),
        None => {
            builder.add_source(config::File::from(defaults::default_config_path()).required(false))
        }
    };

    let settings = builder.add_source(environment).build()?;

    let configuration = settings.try_deserialize::<AppConfiguration>()?;

    configuration.validate()?;

    Ok(configuration)
}

fn environment_source() -> config::Environment {
    configure_environment(config::Environment::with_prefix("RETSU"))
}

fn configure_environment(environment: config::Environment) -> config::Environment {
    environment
        .prefix_separator("_")
        .separator("__")
        .try_parsing(true)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use uuid::Uuid;

    use super::{
        ConfigurationError, configure_environment, load_with_environment, schema::Environment,
    };

    struct TemporaryConfig {
        path: PathBuf,
    }

    impl TemporaryConfig {
        fn new(contents: &str) -> Self {
            let path = std::env::temp_dir().join(format!("retsu-config-{}.yaml", Uuid::new_v4()));

            fs::write(&path, contents).expect("temporary configuration should be written");

            Self { path }
        }
    }

    impl Drop for TemporaryConfig {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn environment(entries: &[(&str, &str)]) -> config::Environment {
        let source = entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>();

        configure_environment(config::Environment::with_prefix("RETSU").source(Some(source)))
    }

    fn expect_load_error(
        result: Result<super::AppConfiguration, ConfigurationError>,
        message: &str,
    ) -> ConfigurationError {
        match result {
            Ok(_) => panic!("{message}"),
            Err(error) => error,
        }
    }

    #[test]
    fn loads_yaml_and_applies_nested_environment_overrides() {
        let file = TemporaryConfig::new(
            r#"
environment: test
http:
  port: 3100
logging:
  format: json
"#,
        );
        let environment = environment(&[
            ("RETSU_ENVIRONMENT", "staging"),
            ("RETSU_HTTP__PORT", "3200"),
            ("RETSU_DATABASE__MAX_CONNECTIONS", "20"),
        ]);

        let configuration = load_with_environment(Some(&file.path), environment)
            .expect("configuration should load");

        assert_eq!(configuration.environment, Environment::Staging);
        assert_eq!(configuration.http.port, 3200);
        assert_eq!(configuration.database.max_connections, 20);
    }

    #[test]
    fn fills_omitted_yaml_values_from_struct_defaults() {
        let file = TemporaryConfig::new("{}");

        let configuration = load_with_environment(Some(&file.path), environment(&[]))
            .expect("defaulted configuration should load");

        assert_eq!(configuration.environment, Environment::Local);
        assert_eq!(configuration.http.port, 2424);
        assert_eq!(configuration.database.max_connections, 10);
        assert_eq!(configuration.worker.shutdown_timeout_seconds, 30);
    }

    #[test]
    fn rejects_unknown_yaml_fields() {
        let file = TemporaryConfig::new(
            r#"
http:
  unexpected: true
"#,
        );

        let error = expect_load_error(
            load_with_environment(Some(&file.path), environment(&[])),
            "unknown fields should fail",
        );

        assert!(matches!(error, ConfigurationError::Load(_)));
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_invalid_nested_environment_values_after_deserialization() {
        let file = TemporaryConfig::new("{}");
        let environment = environment(&[("RETSU_HTTP__PORT", "0")]);

        let error = expect_load_error(
            load_with_environment(Some(&file.path), environment),
            "invalid port should fail validation",
        );

        assert!(matches!(error, ConfigurationError::Validation(_)));
        assert!(error.to_string().contains("http.port"));
    }

    #[test]
    fn requires_an_explicit_configuration_file_to_exist() {
        let missing_path =
            std::env::temp_dir().join(format!("missing-retsu-{}.yaml", Uuid::new_v4()));

        let error = expect_load_error(
            load_with_environment(Some(&missing_path), environment(&[])),
            "missing explicit file should fail",
        );

        assert!(matches!(error, ConfigurationError::Load(_)));
    }
}
