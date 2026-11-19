//! Name resolution for remote sources, with the address policy applied to every answer.
//!
//! A host name is resolved by the server itself, once per connection, and every returned
//! address must pass the policy of [`crate::webdav::is_unsafe_address`]. The connection
//! then goes to one of the validated addresses while the original host name stays in the
//! URL, so TLS SNI, certificate verification and the `Host` header are unchanged. A record
//! that changes between connections is checked again on each new connection.
//!
//! Loopback and private ranges stay allowed: a source is normally a NAS on the same LAN.

use std::{
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
};

use learnvault_core::SourceError;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::webdav::is_unsafe_address;

/// Future returned by [`AddressResolver::lookup`].
pub(crate) type Lookup<'a> = Pin<Box<dyn Future<Output = io::Result<Vec<SocketAddr>>> + Send + 'a>>;

/// Source of host name answers; the seam that tests replace with a fake.
pub(crate) trait AddressResolver: Send + Sync {
    fn lookup<'a>(&'a self, host: &'a str, port: u16) -> Lookup<'a>;
}

/// Resolver backed by the operating system.
#[derive(Debug, Default)]
pub(crate) struct SystemResolver;

impl AddressResolver for SystemResolver {
    fn lookup<'a>(&'a self, host: &'a str, port: u16) -> Lookup<'a> {
        Box::pin(async move { Ok(tokio::net::lookup_host((host, port)).await?.collect()) })
    }
}

/// Resolves `host` and returns its addresses only if every one of them is allowed.
///
/// An IP literal is not looked up; it only goes through the same policy.
pub(crate) async fn resolve_allowed(
    resolver: &dyn AddressResolver,
    host: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, SourceError> {
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    let addresses = match host.parse::<IpAddr>() {
        Ok(address) => vec![SocketAddr::new(address, port)],
        Err(_) => resolver
            .lookup(host, port)
            .await
            .map_err(|_| SourceError::Unreachable)?,
    };
    if addresses.is_empty() {
        return Err(SourceError::Unreachable);
    }
    // Every address is checked: the client may try any of them.
    if addresses
        .iter()
        .any(|address| is_unsafe_address(address.ip()))
    {
        return Err(SourceError::Denied);
    }
    Ok(addresses)
}

/// Adapts a resolver to `reqwest`, so each new connection is resolved and validated.
pub(crate) struct PolicyResolver(pub(crate) Arc<dyn AddressResolver>);

impl Resolve for PolicyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = Arc::clone(&self.0);
        Box::pin(async move {
            // Port 0 makes reqwest use the port of the URL.
            let addresses = resolve_allowed(resolver.as_ref(), name.as_str(), 0)
                .await
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    /// Fake resolver: each host has a list of answers; every lookup consumes the next one
    /// (the last answer repeats).
    pub(crate) struct FakeResolver {
        answers: Mutex<HashMap<String, Vec<Vec<IpAddr>>>>,
        pub(crate) lookups: Mutex<usize>,
    }

    impl FakeResolver {
        pub(crate) fn new(host: &str, answers: Vec<Vec<IpAddr>>) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(HashMap::from([(host.to_owned(), answers)])),
                lookups: Mutex::new(0),
            })
        }
    }

    impl AddressResolver for FakeResolver {
        fn lookup<'a>(&'a self, host: &'a str, port: u16) -> Lookup<'a> {
            Box::pin(async move {
                *self.lookups.lock().unwrap() += 1;
                let mut answers = self.answers.lock().unwrap();
                let list = answers
                    .get_mut(host)
                    .ok_or_else(|| io::Error::other("unknown host"))?;
                let answer = if list.len() > 1 {
                    list.remove(0)
                } else {
                    list[0].clone()
                };
                Ok(answer
                    .into_iter()
                    .map(|address| SocketAddr::new(address, port))
                    .collect())
            })
        }
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[tokio::test]
    async fn a_name_with_only_permitted_addresses_resolves() {
        let resolver = FakeResolver::new("nas.test", vec![vec![ip("192.168.1.20"), ip("::1")]]);
        let addresses = resolve_allowed(resolver.as_ref(), "nas.test", 445)
            .await
            .unwrap();
        assert_eq!(addresses.len(), 2);
        assert!(addresses.iter().all(|address| address.port() == 445));
    }

    #[tokio::test]
    async fn one_denied_address_among_permitted_ones_refuses_the_name() {
        let resolver = FakeResolver::new(
            "nas.test",
            vec![vec![ip("192.168.1.20"), ip("169.254.169.254")]],
        );
        assert_eq!(
            resolve_allowed(resolver.as_ref(), "nas.test", 80)
                .await
                .unwrap_err(),
            SourceError::Denied
        );
    }

    #[tokio::test]
    async fn records_that_change_between_lookups_are_checked_each_time() {
        let resolver = FakeResolver::new(
            "nas.test",
            vec![vec![ip("192.168.1.20")], vec![ip("fe80::1")]],
        );
        assert!(
            resolve_allowed(resolver.as_ref(), "nas.test", 80)
                .await
                .is_ok()
        );
        assert_eq!(
            resolve_allowed(resolver.as_ref(), "nas.test", 80)
                .await
                .unwrap_err(),
            SourceError::Denied
        );
    }

    #[tokio::test]
    async fn literal_ips_skip_the_lookup_and_keep_the_policy() {
        let resolver = FakeResolver::new("nas.test", vec![vec![ip("10.0.0.1")]]);
        let lan = resolve_allowed(resolver.as_ref(), "10.0.0.5", 445)
            .await
            .unwrap();
        assert_eq!(lan, vec!["10.0.0.5:445".parse().unwrap()]);
        let v6 = resolve_allowed(resolver.as_ref(), "[::1]", 445)
            .await
            .unwrap();
        assert_eq!(v6, vec!["[::1]:445".parse().unwrap()]);
        assert_eq!(
            resolve_allowed(resolver.as_ref(), "169.254.169.254", 80)
                .await
                .unwrap_err(),
            SourceError::Denied
        );
        assert_eq!(*resolver.lookups.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn lookup_failures_and_empty_answers_are_unreachable() {
        let resolver = FakeResolver::new("nas.test", vec![vec![]]);
        for host in ["nas.test", "missing.test"] {
            assert_eq!(
                resolve_allowed(resolver.as_ref(), host, 80)
                    .await
                    .unwrap_err(),
                SourceError::Unreachable
            );
        }
    }
}
