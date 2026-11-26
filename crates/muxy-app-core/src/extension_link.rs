use muxy_core::release::Channel;

const MAX_LINK: usize = 2048;
const INSTALL: &str = "//extensions/install/";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionLink {
    pub name: String,
}

impl ExtensionLink {
    pub fn parse(link: &str) -> Option<Self> {
        Self::parse_in(Channel::current(), link)
    }

    fn parse_in(channel: Channel, link: &str) -> Option<Self> {
        if link.len() > MAX_LINK {
            return None;
        }
        let (scheme, rest) = link.split_once(':')?;
        if !scheme.eq_ignore_ascii_case(channel.url_scheme()) {
            return None;
        }
        let rest = rest.split_once('#').map_or(rest, |(rest, _)| rest);
        let path = rest.split_once('?').map_or(rest, |(path, _)| path);
        if !path.get(..INSTALL.len())?.eq_ignore_ascii_case(INSTALL) {
            return None;
        }
        let name = &path[INSTALL.len()..];
        let name = decode(name.strip_suffix('/').unwrap_or(name))?;
        crate::extensions::identifier(&name).then_some(Self { name })
    }
}

fn decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.bytes();
    while let Some(byte) = rest.next() {
        if byte == b'%' {
            let high = char::from(rest.next()?).to_digit(16)?;
            let low = char::from(rest.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str) -> ExtensionLink {
        ExtensionLink { name: name.into() }
    }

    #[test]
    fn store_links_open_the_extension_in_their_own_channel_only() {
        for (channel, text) in [
            (Channel::Stable, "muxy://extensions/install/git-status"),
            (Channel::Beta, "muxy-beta://extensions/install/git-status"),
            (Channel::Stable, "MUXY://Extensions/Install/git-status/"),
            (Channel::Stable, "muxy://extensions/install/git%2Dstatus"),
            (
                Channel::Stable,
                "muxy://extensions/install/git-status?ref=store#top",
            ),
        ] {
            assert_eq!(
                ExtensionLink::parse_in(channel, text),
                Some(named("git-status")),
                "{text}"
            );
        }
        assert_eq!(
            ExtensionLink::parse_in(Channel::Beta, "muxy://extensions/install/git-status"),
            None
        );
        assert_eq!(
            ExtensionLink::parse_in(Channel::Stable, "muxy-beta://extensions/install/git-status"),
            None
        );
    }

    #[test]
    fn links_that_name_no_valid_extension_are_refused() {
        let long = format!("muxy://extensions/install/{}", "a".repeat(MAX_LINK));
        for text in [
            "muxy://extensions/install",
            "muxy://extensions/install/",
            "muxy://extensions/git-status",
            "muxy://extensions/install/git/status",
            "muxy://extensions/install/git%2Fstatus",
            "muxy://extensions/install/..",
            "muxy://extensions/install/.hidden",
            "muxy://extensions/install/bad%zz",
            "muxy://extensions/install/bad%2",
            "muxy://session/7",
            "https://extensions/install/git-status",
            &long,
        ] {
            assert_eq!(
                ExtensionLink::parse_in(Channel::Stable, text),
                None,
                "{text}"
            );
        }
    }
}
