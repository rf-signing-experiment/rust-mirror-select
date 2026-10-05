use crate::{Error, MirrorSet};

impl MirrorSet {
    pub fn with_toml(data: impl AsRef<str>) -> Result<Self, Error> {
        Ok(::toml::from_str(data.as_ref())?)
    }

    pub fn to_toml(&self) -> Result<String, Error> {
        Ok(::toml::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mirror, MirrorContents};

    const DOCUMENT: &str = r#"
[[mirrors]]
[mirrors.V1]
url = "https://static.rust-lang.org/"
contains = 15

[[mirrors]]
[mirrors.V1]
url = "https://mirror.example/rustup/"
contains = 12
"#;

    fn set() -> MirrorSet {
        [
            ("https://static.rust-lang.org/", MirrorContents::all()),
            (
                "https://mirror.example/rustup/",
                MirrorContents::TOOLCHAIN | MirrorContents::COMPONENT,
            ),
        ]
        .into_iter()
        .map(|(url, contains)| Mirror::new(url.parse().unwrap(), contains))
        .collect()
    }

    #[test]
    fn parses_document() {
        assert_eq!(MirrorSet::with_toml(DOCUMENT).unwrap(), set());
    }

    #[test]
    fn round_trips() {
        let text = set().to_toml().unwrap();
        assert_eq!(MirrorSet::with_toml(&text).unwrap(), set());
    }

    #[test]
    fn empty_document_is_rejected() {
        assert!(matches!(MirrorSet::with_toml(""), Err(Error::TomlParse(_))));
    }

    #[test]
    fn unknown_contents_bits_are_rejected() {
        let text = DOCUMENT.replace("contains = 15", "contains = 200");
        assert!(matches!(
            MirrorSet::with_toml(text),
            Err(Error::TomlParse(_))
        ));
    }
}
