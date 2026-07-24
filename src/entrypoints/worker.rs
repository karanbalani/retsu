use crate::configuration::AppConfiguration;

pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    println!("worker mode {}", configuration.environment);
    Ok(())
}
