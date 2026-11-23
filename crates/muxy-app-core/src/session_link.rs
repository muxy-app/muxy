use muxy_core::release::Channel;
use muxy_protocol::{ServerIdentity, SessionId};

use crate::Direction;

const MAX_LINK: usize = 2048;
const SESSION: &str = "//session/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionLink {
    pub session: SessionId,
    pub server: Option<ServerIdentity>,
    pub split: Option<Direction>,
}

impl SessionLink {
    pub fn parse(link: &str) -> Option<Self> {
        Self::parse_in(Channel::current(), link)
    }

    pub fn to_link(&self) -> String {
        self.link_in(Channel::current())
    }

    fn link_in(&self, channel: Channel) -> String {
        let mut link = format!("{}:{SESSION}{}", channel.url_scheme(), self.session.get());
        let mut separator = '?';
        if let Some(server) = self.server {
            link.push(separator);
            link.push_str("server=");
            link.push_str(&server.to_string());
            separator = '&';
        }
        if let Some(split) = self.split.and_then(split_name) {
            link.push(separator);
            link.push_str("split=");
            link.push_str(split);
        }
        link
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
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        if !path.get(..SESSION.len())?.eq_ignore_ascii_case(SESSION) {
            return None;
        }
        let id = &path[SESSION.len()..];
        let id = id.strip_suffix('/').unwrap_or(id);
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let mut parsed = Self {
            session: SessionId::new(id.parse().ok()?)?,
            server: None,
            split: None,
        };
        let mut split_given = false;
        for field in query.split('&').filter(|field| !field.is_empty()) {
            let (key, value) = field.split_once('=').unwrap_or((field, ""));
            match key {
                "server" => {
                    if parsed.server.replace(value.parse().ok()?).is_some() {
                        return None;
                    }
                }
                "split" => {
                    if std::mem::replace(&mut split_given, true) {
                        return None;
                    }
                    parsed.split = match value {
                        "right" => Some(Direction::Right),
                        "down" => Some(Direction::Down),
                        _ => None,
                    };
                }
                _ => {}
            }
        }
        Some(parsed)
    }
}

fn split_name(direction: Direction) -> Option<&'static str> {
    match direction {
        Direction::Right => Some("right"),
        Direction::Down => Some("down"),
        Direction::Left | Direction::Up => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: u64) -> SessionId {
        SessionId::new(id).expect("session")
    }

    fn link(id: u64, server: Option<u128>, split: Option<Direction>) -> SessionLink {
        SessionLink {
            session: session(id),
            server: server.map(ServerIdentity::from_u128),
            split,
        }
    }

    #[test]
    fn a_printed_link_opens_the_same_session_in_its_own_channel_only() {
        for channel in [Channel::Stable, Channel::Beta] {
            let other = match channel {
                Channel::Stable => Channel::Beta,
                Channel::Beta => Channel::Stable,
            };
            for expected in [
                link(7, None, None),
                link(7, Some(0xab), None),
                link(42, None, Some(Direction::Down)),
                link(u64::MAX, Some(1), Some(Direction::Right)),
            ] {
                let printed = expected.link_in(channel);
                assert_eq!(SessionLink::parse_in(channel, &printed), Some(expected));
                assert_eq!(SessionLink::parse_in(other, &printed), None, "{printed}");
            }
        }
        assert_eq!(
            link(7, Some(0xab), Some(Direction::Right)).link_in(Channel::Stable),
            "muxy://session/7?server=000000000000000000000000000000ab&split=right"
        );
    }

    #[test]
    fn links_from_browsers_and_newer_apps_still_open() {
        let server = "000000000000000000000000000000ab";
        for (text, expected) in [
            ("MUXY://Session/7/", link(7, None, None)),
            (
                "muxy://session/7/?split=down#top",
                link(7, None, Some(Direction::Down)),
            ),
            ("muxy://session/7?split=left", link(7, None, None)),
            (
                "muxy://session/7?tab=new&&split=right",
                link(7, None, Some(Direction::Right)),
            ),
            (
                &format!("muxy://session/7?server={server}"),
                link(7, Some(0xab), None),
            ),
        ] {
            assert_eq!(
                SessionLink::parse_in(Channel::Stable, text),
                Some(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn links_that_could_reach_the_wrong_session_are_refused() {
        let long = format!("muxy://session/7?note={}", "a".repeat(MAX_LINK));
        for text in [
            "muxy://session/",
            "muxy://session/0",
            "muxy://session/-7",
            "muxy://session/+7",
            "muxy://session/7x",
            "muxy://session/7/8",
            "muxy://session/18446744073709551616",
            "muxy://sessions/7",
            "muxy://pair?v=1",
            "muxy:session/7",
            "https://session/7",
            "muxy://session/7?server=devbox",
            "muxy://session/7?server=",
            "muxy://session/7?server=000000000000000000000000000000ab&server=000000000000000000000000000000ac",
            "muxy://session/7?split=right&split=down",
            &long,
        ] {
            assert_eq!(SessionLink::parse_in(Channel::Stable, text), None, "{text}");
        }
    }
}
