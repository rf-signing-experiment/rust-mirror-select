use serde::{Deserialize, Serialize};

use crate::{Error, Mirror, MirrorEntry, MirrorSelectClient, SelectResolver};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MirrorSet {
    mirrors: Vec<Mirror>,
}

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

    pub fn from_entries<I, T>(entries: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        entries
            .into_iter()
            .map(|text| MirrorEntry::from_base64(text)?.decode())
            .collect()
    }

    pub fn to_entries(&self) -> Result<Vec<MirrorEntry>, Error> {
        self.mirrors.iter().map(MirrorEntry::encode).collect()
    }

    pub fn mirrors(&self) -> &[Mirror] {
        &self.mirrors
    }

    pub fn len(&self) -> usize {
        self.mirrors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mirrors.is_empty()
    }
}

impl FromIterator<Mirror> for MirrorSet {
    fn from_iter<I: IntoIterator<Item = Mirror>>(iter: I) -> Self {
        Self {
            mirrors: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MirrorContents;

    #[test]
    fn mirror_set_round_trips_through_entries() {
        let set: MirrorSet = ["https://a.example/", "https://b.example/rustup/"]
            .into_iter()
            .map(|url| Mirror::new(url.parse().unwrap(), MirrorContents::all()))
            .collect();

        let entries: Vec<String> = set
            .to_entries()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(MirrorSet::from_entries(&entries).unwrap(), set);
    }

    #[test]
    fn empty_entries_give_empty_set() {
        let set = MirrorSet::from_entries(Vec::<&str>::new()).unwrap();
        assert!(set.is_empty());
    }
}
