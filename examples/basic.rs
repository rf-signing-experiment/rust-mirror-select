use std::env;

use rust_mirror_select::{DEFAULT_DNS_NAME, Error, MirrorSet};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let name = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_DNS_NAME.to_owned());
    let set = MirrorSet::from_dns(&name).await?;
    println!("{set:#?}");
    Ok(())
}
