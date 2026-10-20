//! `kartenn-dev-idp`, development only:
//! - `kartenn-dev-idp certs <dir>`: the authority, the certificate of the loopback, the
//!   client secret and the key ring of kartenn-server, written once;
//! - `kartenn-dev-idp serve <config-file>`: the issuer, until `Ctrl-C`.

use std::path::Path;
use std::process::ExitCode;

use kartenn_dev_idp::config::Config;
use kartenn_dev_idp::server::Server;

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, dir] if command == "certs" => {
            kartenn_dev_idp::certs::generate(Path::new(dir))?;
        }
        [command, file] if command == "serve" => {
            let config = Config::load(Path::new(file))?;
            let (issuer, listen) = (config.issuer.clone(), config.listen);
            let server = Server::new(config)?;
            eprintln!("kartenn-dev-idp: {issuer} on {listen}");
            server
                .run(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
        }
        _ => {
            eprintln!(
                "usage: kartenn-dev-idp certs <dir>\n       kartenn-dev-idp serve <config-file>"
            );
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}
