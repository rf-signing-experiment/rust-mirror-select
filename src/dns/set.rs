use crate::dns::{MirrorSelectClient, SelectResolver};
use crate::{Error, MirrorSet};

impl MirrorSet {
    pub async fn from_dns(name: &str) -> Result<Self, Error> {
        Self::with_resolver(&SelectResolver::system()?, name).await
    }

    pub async fn with_resolver(resolver: &SelectResolver, name: &str) -> Result<Self, Error> {
        Self::from_entries(resolver.lookup_txt(name).await?)
    }

    pub async fn with_dns_record(client: &MirrorSelectClient, name: &str) -> Result<Self, Error> {
        Self::from_entries(client.lookup_txt(name).await?)
    }
}
