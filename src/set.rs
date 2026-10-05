use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{Error, Mirror, MirrorEntry};

pub trait SelectStrategy {
    fn get_best<'a>(
        &mut self,
        mirrors: &'a MirrorSet,
    ) -> impl Future<Output = Result<&'a Mirror, Error>> + Send;
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorSet {
    mirrors: Vec<Mirror>,
}

impl MirrorSet {
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
