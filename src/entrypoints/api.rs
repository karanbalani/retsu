use crate::configuration::AppConfiguration;

pub(crate) async fn run(configuration: AppConfiguration) -> anyhow::Result<()> {
    println!("api mode: {}", configuration.http.socket_address());
    Ok(())
}
