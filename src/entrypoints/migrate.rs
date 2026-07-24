use crate::configuration::AppConfiguration;

pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    println!("migrate mode {}", configuration.environment);
    Ok(())
}
