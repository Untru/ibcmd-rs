//! A SQL Server name the way `sqlcmd -S` and the 1C platform spell it:
//! `host`, `host\instance`, `host,port`, `host\instance,port`, an optional
//! `tcp:` prefix, and `.` / `(local)` for this machine.

use anyhow::{Context, Result, bail};

/// The port SQL Server's default instance listens on.
pub const DEFAULT_PORT: u16 = 1433;

/// Where one SQL Server instance is reached over TCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress {
    /// Host name or IPv4 address (`.` and `(local)` read as `localhost`).
    pub host: String,
    /// A named instance, resolved through the SQL Browser service (UDP 1434)
    /// unless a port is given. The default instance (`MSSQLSERVER`) is none.
    pub instance: Option<String>,
    /// An explicit TCP port; it wins over the instance name, as in sqlcmd.
    pub port: Option<u16>,
}

impl ServerAddress {
    pub fn parse(server: &str) -> Result<Self> {
        let text = server.trim();
        if text.is_empty() {
            bail!("the SQL Server name is empty");
        }
        let text = match text.split_once(':') {
            Some((protocol, rest)) if protocol.eq_ignore_ascii_case("tcp") => rest.trim(),
            Some((protocol, _))
                if ["np", "lpc", "admin", "via"]
                    .iter()
                    .any(|name| protocol.eq_ignore_ascii_case(name)) =>
            {
                bail!(
                    "SQL Server name {server:?}: the {protocol}: protocol is not supported by the \
                     built-in client, which connects over TCP (use host, host\\instance or \
                     host,port, or pass --sqlcmd to run sqlcmd.exe)"
                )
            }
            _ => text,
        };
        let (name, port) = match text.rsplit_once(',') {
            Some((name, port)) => {
                let port = port.trim();
                let port = port
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .with_context(|| {
                        format!("SQL Server name {server:?}: {port:?} is not a TCP port")
                    })?;
                (name.trim(), Some(port))
            }
            None => (text, None),
        };
        let (host, instance) = match name.split_once('\\') {
            Some((host, instance)) => (host.trim(), Some(instance.trim())),
            None => (name, None),
        };
        if host.is_empty() {
            bail!("SQL Server name {server:?} has no host");
        }
        if host.contains(['\\', ',', ' ']) {
            bail!("SQL Server name {server:?} is not host[\\instance][,port]");
        }
        let host = if host == "." || host.eq_ignore_ascii_case("(local)") {
            "localhost".to_owned()
        } else {
            host.to_owned()
        };
        let instance = match instance {
            Some("") => bail!("SQL Server name {server:?} has an empty instance name"),
            Some(instance) if instance.contains(['\\', ',']) => {
                bail!("SQL Server name {server:?} is not host[\\instance][,port]")
            }
            // `MSSQLSERVER` names the default instance, which has no browser
            // entry of its own.
            Some(instance) if instance.eq_ignore_ascii_case("MSSQLSERVER") => None,
            Some(instance) => Some(instance.to_owned()),
            None => None,
        };
        Ok(Self {
            host,
            instance,
            port,
        })
    }

    /// The instance name the SQL Browser must resolve, when no port says
    /// where the instance listens.
    pub fn browser_instance(&self) -> Option<&str> {
        match self.port {
            Some(_) => None,
            None => self.instance.as_deref(),
        }
    }

    /// The TCP port to connect to when no SQL Browser lookup is needed.
    pub fn direct_port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }
}

impl std::fmt::Display for ServerAddress {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.host)?;
        if let Some(instance) = &self.instance {
            write!(formatter, "\\{instance}")?;
        }
        if let Some(port) = self.port {
            write!(formatter, ",{port}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ServerAddress;

    fn parsed(server: &str) -> (String, Option<String>, Option<u16>) {
        let address = ServerAddress::parse(server).unwrap();
        (address.host, address.instance, address.port)
    }

    #[test]
    fn plain_hosts_and_local_aliases() {
        assert_eq!(parsed("localhost"), ("localhost".into(), None, None));
        assert_eq!(parsed("  db-01.corp  "), ("db-01.corp".into(), None, None));
        assert_eq!(parsed("."), ("localhost".into(), None, None));
        assert_eq!(parsed("(local)"), ("localhost".into(), None, None));
        assert_eq!(parsed("(LOCAL)"), ("localhost".into(), None, None));
        assert_eq!(parsed("10.0.0.5"), ("10.0.0.5".into(), None, None));
    }

    #[test]
    fn named_instances_and_ports() {
        assert_eq!(
            parsed("sql01\\ERP"),
            ("sql01".into(), Some("ERP".into()), None)
        );
        assert_eq!(
            parsed(".\\SQLEXPRESS"),
            ("localhost".into(), Some("SQLEXPRESS".into()), None)
        );
        assert_eq!(parsed("sql01,14330"), ("sql01".into(), None, Some(14330)));
        assert_eq!(
            parsed("sql01\\ERP,14330"),
            ("sql01".into(), Some("ERP".into()), Some(14330))
        );
        assert_eq!(parsed("sql01 , 1500"), ("sql01".into(), None, Some(1500)));
        // The default instance has no SQL Browser entry.
        assert_eq!(parsed("sql01\\MSSQLSERVER"), ("sql01".into(), None, None));
        assert_eq!(parsed("sql01\\mssqlserver"), ("sql01".into(), None, None));
    }

    #[test]
    fn tcp_prefix_is_accepted() {
        assert_eq!(parsed("tcp:localhost"), ("localhost".into(), None, None));
        assert_eq!(parsed("TCP:sql01,1433"), ("sql01".into(), None, Some(1433)));
        assert_eq!(
            parsed("tcp:sql01\\ERP"),
            ("sql01".into(), Some("ERP".into()), None)
        );
    }

    #[test]
    fn a_port_wins_over_the_sql_browser() {
        let named = ServerAddress::parse("sql01\\ERP").unwrap();
        assert_eq!(named.browser_instance(), Some("ERP"));
        let pinned = ServerAddress::parse("sql01\\ERP,50123").unwrap();
        assert_eq!(pinned.browser_instance(), None);
        assert_eq!(pinned.direct_port(), 50123);
        assert_eq!(
            ServerAddress::parse("sql01").unwrap().direct_port(),
            super::DEFAULT_PORT
        );
    }

    #[test]
    fn display_round_trips_the_parsed_form() {
        for server in ["localhost", "sql01\\ERP", "sql01,1500", "sql01\\ERP,1500"] {
            assert_eq!(ServerAddress::parse(server).unwrap().to_string(), server);
        }
    }

    #[test]
    fn malformed_names_are_refused() {
        for server in [
            "",
            "   ",
            ",1433",
            "\\ERP",
            "sql01,",
            "sql01,port",
            "sql01,0",
            "sql01,70000",
            "sql01\\",
            "sql01\\a\\b",
            "sql 01",
        ] {
            assert!(ServerAddress::parse(server).is_err(), "{server:?}");
        }
    }

    #[test]
    fn other_protocols_are_refused_with_a_hint() {
        for server in ["np:\\\\.\\pipe\\sql\\query", "lpc:localhost", "admin:sql01"] {
            let error = ServerAddress::parse(server).unwrap_err().to_string();
            assert!(error.contains("not supported"), "{error}");
            assert!(error.contains("--sqlcmd"), "{error}");
        }
    }
}
