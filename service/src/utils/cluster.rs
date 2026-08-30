//! Database cluster connection helpers.
//!
//! `DataSourceConnection.host` accepts a comma-separated host list — plain
//! hosts sharing the connection `port` (`"pg1,pg2"`), or per-host ports
//! (`"pg1:5433,pg2:5432"`). The PostGIS / MySQL / MongoDB pool builders parse
//! this into a failover host list; MongoDB additionally accepts a full
//! `mongodb://` URI in `host` plus a `replica_set` name.

use crate::models::DataSourceConnection;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A resolved cluster endpoint: host + port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterHost {
    pub host: String,
    pub port: u16,
}

/// True when the host field carries a full connection string rather than a
/// host (postgres:// · postgresql:// · mysql:// · mongodb:// · mongodb+srv://).
pub fn is_connection_string(host: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    h.starts_with("postgres://")
        || h.starts_with("postgresql://")
        || h.starts_with("mysql://")
        || h.starts_with("mongodb://")
        || h.starts_with("mongodb+srv://")
}

/// Parse the comma-separated host list of a `DataSourceConnection`.
///
/// - `"pg1,pg2"` → both hosts at `fallback_port` (the connection's `port`
///   field) or `default_port`
/// - `"pg1:5433,pg2"` → per-host ports; entries without a port keep the
///   shared port
/// - `"localhost"` entries are rewritten to `127.0.0.1` (IPv6 resolution guard)
/// - an empty/absent host yields a single `(127.0.0.1, default_port)` entry
pub fn parse_host_list(
    host: Option<&str>,
    fallback_port: Option<u16>,
    default_port: u16,
) -> Vec<ClusterHost> {
    let raw = host.unwrap_or("").trim();
    if raw.is_empty() {
        return vec![ClusterHost {
            host: "127.0.0.1".to_string(),
            port: default_port,
        }];
    }
    let shared_port = fallback_port.unwrap_or(default_port);
    let mut out: Vec<ClusterHost> = Vec::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        // "host:port" — a single ':' means host + port; bracketed IPv6
        // ("[::1]:5433" / "[::1]") is split after the closing bracket.
        let (h, port): (String, Option<u16>) = if let Some(rest) = entry.strip_prefix('[') {
            match rest.split_once(']') {
                Some((inner, tail)) => {
                    let port = tail.strip_prefix(':').and_then(|p| p.parse::<u16>().ok());
                    (format!("[{}]", inner), port)
                },
                None => (entry.to_string(), None),
            }
        } else if entry.matches(':').count() == 1 {
            let (h, p) = entry.split_once(':').expect("exactly one ':'");
            (h.to_string(), p.parse::<u16>().ok())
        } else {
            (entry.to_string(), None)
        };
        let host = if h.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_string()
        } else {
            h.to_string()
        };
        out.push(ClusterHost {
            host,
            port: port.unwrap_or(shared_port),
        });
    }
    if out.is_empty() {
        vec![ClusterHost {
            host: "127.0.0.1".to_string(),
            port: default_port,
        }]
    } else {
        out
    }
}

/// Probe the host list in order with a short TCP connect and return the first
/// reachable endpoint. MySQL failover: `mysql_async` pools are single-host, so
/// the pool is pinned to the first reachable node at build time.
pub fn first_reachable_tcp(hosts: &[ClusterHost], timeout: Duration) -> Option<ClusterHost> {
    for h in hosts {
        let addr = format!("{}:{}", h.host, h.port);
        let Ok(mut addrs) = addr.to_socket_addrs() else {
            continue;
        };
        let Some(sock_addr) = addrs.next() else {
            continue;
        };
        if TcpStream::connect_timeout(&sock_addr, timeout).is_ok() {
            return Some(h.clone());
        }
    }
    None
}

/// Percent-encode a URI component (MongoDB connection-string
/// username/password).
pub fn uri_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            },
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Build the MongoDB connection URI from a `DataSourceConnection`.
///
/// - `host` starting with `mongodb://` / `mongodb+srv://` is used verbatim
///   (full URI override, replica set included in the query string)
/// - otherwise the comma-separated host list becomes the seed list
///   (`mongodb://u:p@h1:p1,h2:p2/db?replicaSet=rs0` when `replica_set` is set)
pub fn mongo_uri_from_connection(conn: &DataSourceConnection) -> String {
    let host_raw = conn.host.as_deref().unwrap_or("127.0.0.1").trim();
    if is_connection_string(host_raw) {
        return host_raw.to_string();
    }
    let hosts = parse_host_list(conn.host.as_deref(), conn.port, 27017);
    let host_part = hosts
        .iter()
        .map(|h| format!("{}:{}", h.host, h.port))
        .collect::<Vec<_>>()
        .join(",");
    let database = conn.database.as_deref().unwrap_or("geoserver");
    let creds = match conn.username.as_deref() {
        Some(u) => format!(
            "{}:{}@",
            uri_encode(u),
            uri_encode(conn.password.as_deref().unwrap_or(""))
        ),
        None => String::new(),
    };
    let rs = match conn.replica_set.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(r) => format!("?replicaSet={}", uri_encode(r.trim())),
        None => String::new(),
    };
    format!("mongodb://{}{}/{}{}", creds, host_part, database, rs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_host() {
        let hosts = parse_host_list(Some("db.example.com"), Some(5433), 5432);
        assert_eq!(
            hosts,
            vec![ClusterHost {
                host: "db.example.com".into(),
                port: 5433
            }]
        );
    }

    #[test]
    fn test_parse_defaults() {
        let hosts = parse_host_list(None, None, 3306);
        assert_eq!(
            hosts,
            vec![ClusterHost {
                host: "127.0.0.1".into(),
                port: 3306
            }]
        );
    }

    #[test]
    fn test_parse_comma_separated_shares_port() {
        let hosts = parse_host_list(Some("pg1,pg2, pg3"), Some(5433), 5432);
        assert_eq!(
            hosts,
            vec![
                ClusterHost {
                    host: "pg1".into(),
                    port: 5433
                },
                ClusterHost {
                    host: "pg2".into(),
                    port: 5433
                },
                ClusterHost {
                    host: "pg3".into(),
                    port: 5433
                },
            ]
        );
    }

    #[test]
    fn test_parse_per_host_ports() {
        let hosts = parse_host_list(Some("pg1:5433,pg2:5434,pg3"), Some(5432), 5432);
        assert_eq!(
            hosts,
            vec![
                ClusterHost {
                    host: "pg1".into(),
                    port: 5433
                },
                ClusterHost {
                    host: "pg2".into(),
                    port: 5434
                },
                ClusterHost {
                    host: "pg3".into(),
                    port: 5432
                },
            ]
        );
    }

    #[test]
    fn test_parse_localhost_and_ipv6() {
        let hosts = parse_host_list(Some("localhost,[::1]:5433"), None, 5432);
        assert_eq!(
            hosts,
            vec![
                ClusterHost {
                    host: "127.0.0.1".into(),
                    port: 5432
                },
                ClusterHost {
                    host: "[::1]".into(),
                    port: 5433
                },
            ]
        );
    }

    #[test]
    fn test_is_connection_string() {
        assert!(is_connection_string("postgres://u:p@h/db"));
        assert!(is_connection_string("PostgreSQL://h/db"));
        assert!(is_connection_string("mongodb+srv://cluster.example.com/db"));
        assert!(!is_connection_string("pg1,pg2"));
        assert!(!is_connection_string("localhost:5432"));
    }

    #[test]
    fn test_mongo_uri_single_and_cluster() {
        let conn = DataSourceConnection {
            host: Some("mongo1".into()),
            port: Some(27018),
            database: Some("gis".into()),
            ..Default::default()
        };
        assert_eq!(
            mongo_uri_from_connection(&conn),
            "mongodb://mongo1:27018/gis"
        );

        let conn = DataSourceConnection {
            host: Some("mongo1:27018,mongo2".into()),
            port: Some(27018),
            database: Some("gis".into()),
            username: Some("user".into()),
            password: Some("p@ss".into()),
            replica_set: Some("rs0".into()),
            ..Default::default()
        };
        assert_eq!(
            mongo_uri_from_connection(&conn),
            "mongodb://user:p%40ss@mongo1:27018,mongo2:27018/gis?replicaSet=rs0"
        );
    }

    #[test]
    fn test_mongo_uri_direct_override() {
        let conn = DataSourceConnection {
            host: Some("mongodb+srv://cluster.example.com/gis?replicaSet=rs0".into()),
            ..Default::default()
        };
        assert_eq!(
            mongo_uri_from_connection(&conn),
            "mongodb+srv://cluster.example.com/gis?replicaSet=rs0"
        );
    }
}
