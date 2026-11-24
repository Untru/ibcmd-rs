//! A PostgreSQL client of its own (#368): the frontend/backend protocol 3.0
//! over TCP, enough to read the configuration tables of an infobase -- the
//! startup, the password logins a server asks for (none, cleartext, MD5,
//! SCRAM-SHA-256) and the simple query protocol, its rows streamed as they
//! arrive. With sslmode=verify-full the session uses authenticated TLS.
//!
//! The protocol is PostgreSQL's documented one ("Frontend/Backend Protocol",
//! chapter 53 of the PostgreSQL 17 manual); SCRAM-SHA-256 is RFC 5802/7677.

use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};

/// Where a PostgreSQL database is and who logs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgTarget {
    pub host: String,
    pub port: u16,
    pub verify_full: bool,
    pub database: String,
    pub user: String,
    pub password: Option<String>,
}

impl PgTarget {
    /// The DBMS server the way 1C names it: `host`, `host:port` or
    /// `host port=N` (the cluster's database server field).
    pub fn new(
        server: &str,
        database: &str,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self> {
        let mut parts = server.trim().trim_end_matches(';').split_ascii_whitespace();
        let address = parts.next().unwrap_or_default().trim_end_matches(';');
        let (host, mut port) = match address.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, Some(port)),
            _ => (address, None),
        };
        let mut verify_full = false;
        for option in parts {
            match option.trim_end_matches(';').split_once('=') {
                Some(("port", value)) if port.is_none() => port = Some(value),
                Some(("sslmode", "verify-full")) if !verify_full => verify_full = true,
                _ => bail!("unsupported or duplicate PostgreSQL server option"),
            }
        }
        let port = match port {
            Some(port) => port
                .trim()
                .parse()
                .with_context(|| format!("PostgreSQL server `{server}`: bad port"))?,
            None => 5432,
        };
        Ok(Self {
            host: if host.is_empty() { "localhost" } else { host }.to_owned(),
            port,
            verify_full,
            database: database.to_owned(),
            user: user.unwrap_or("postgres").to_owned(),
            password: password.map(str::to_owned),
        })
    }
}

/// One open session.
pub struct PgConnection {
    stream: BufReader<Box<dyn Transport>>,
}

trait Transport: Read + Write + Send + Sync {}
impl<T: Read + Write + Send + Sync> Transport for T {}

fn tls_stream(mut socket: TcpStream, host: &str) -> Result<Box<dyn Transport>> {
    socket.write_all(&[0, 0, 0, 8, 4, 210, 22, 47])?;
    let mut response = [0];
    socket.read_exact(&mut response)?;
    if response != [b'S'] {
        bail!("PostgreSQL refused TLS");
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    if roots.is_empty() {
        bail!("no trusted TLS roots for PostgreSQL");
    }
    let config = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::aws_lc_rs::default_provider().into(),
    )
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned())?;
    let client = rustls::ClientConnection::new(config.into(), name)?;
    Ok(Box::new(rustls::StreamOwned::new(client, socket)))
}

/// A server message: its type byte and body.
struct Message {
    tag: u8,
    body: Vec<u8>,
}

impl PgConnection {
    pub fn connect(target: &PgTarget) -> Result<Self> {
        let stream =
            TcpStream::connect((target.host.as_str(), target.port)).with_context(|| {
                format!(
                    "failed to connect to PostgreSQL {}:{}",
                    target.host, target.port
                )
            })?;
        stream.set_nodelay(true).ok();
        let timeout = Some(Duration::from_secs(600));
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        let stream: Box<dyn Transport> = if target.verify_full {
            tls_stream(stream, &target.host)?
        } else {
            Box::new(stream)
        };
        let mut connection = Self {
            stream: BufReader::with_capacity(1 << 20, stream),
        };
        connection.startup(target)?;
        Ok(connection)
    }

    fn startup(&mut self, target: &PgTarget) -> Result<()> {
        let mut body = Vec::new();
        body.extend_from_slice(&196_608u32.to_be_bytes()); // protocol 3.0
        for (key, value) in [
            ("user", target.user.as_str()),
            ("database", target.database.as_str()),
            ("client_encoding", "UTF8"),
            ("application_name", "ibcmd-rs"),
        ] {
            put_cstr(&mut body, key);
            put_cstr(&mut body, value);
        }
        body.push(0);
        let mut packet = ((body.len() + 4) as u32).to_be_bytes().to_vec();
        packet.extend_from_slice(&body);
        self.stream.get_mut().write_all(&packet)?;
        self.stream.get_mut().flush()?;
        loop {
            let message = self.read_message()?;
            match message.tag {
                b'R' => self.authenticate(target, &message.body)?,
                b'E' => bail!(
                    "PostgreSQL refused the login: {}",
                    error_text(&message.body)
                ),
                b'Z' => return Ok(()),
                // ParameterStatus, BackendKeyData, NoticeResponse.
                b'S' | b'K' | b'N' => {}
                tag => bail!("unexpected PostgreSQL message `{}` at login", tag as char),
            }
        }
    }

    fn authenticate(&mut self, target: &PgTarget, body: &[u8]) -> Result<()> {
        let code = be_u32(body, 0)?;
        let password = || {
            target.password.as_deref().ok_or_else(|| {
                anyhow!(
                    "PostgreSQL asks for the password of `{}`: pass --db-pwd",
                    target.user
                )
            })
        };
        match code {
            0 => Ok(()),
            3 => {
                let mut reply = Vec::new();
                put_cstr(&mut reply, password()?);
                self.send(b'p', &reply)
            }
            5 => {
                let salt = body.get(4..8).context("MD5 request without a salt")?;
                let inner = hex(&md5::compute(format!("{}{}", password()?, target.user)).0);
                let mut outer = inner.into_bytes();
                outer.extend_from_slice(salt);
                let mut reply = Vec::new();
                put_cstr(&mut reply, &format!("md5{}", hex(&md5::compute(outer).0)));
                self.send(b'p', &reply)
            }
            10 => self.scram(password()?, &body[4..]),
            // SCRAM continue/final arrive inside `scram`.
            other => bail!("unsupported PostgreSQL authentication method {other}"),
        }
    }

    /// SCRAM-SHA-256 (RFC 7677) without channel binding.
    fn scram(&mut self, password: &str, mechanisms: &[u8]) -> Result<()> {
        let offered = mechanisms
            .split(|byte| *byte == 0)
            .any(|name| name == b"SCRAM-SHA-256");
        if !offered {
            bail!("PostgreSQL offers no SCRAM-SHA-256 login");
        }
        let nonce = base64_encode(uuid::Uuid::new_v4().as_bytes());
        let client_first_bare = format!("n=,r={nonce}");
        let client_first = format!("n,,{client_first_bare}");
        let mut initial = Vec::new();
        put_cstr(&mut initial, "SCRAM-SHA-256");
        initial.extend_from_slice(&(client_first.len() as u32).to_be_bytes());
        initial.extend_from_slice(client_first.as_bytes());
        self.send(b'p', &initial)?;

        let server_first = self.sasl_step(11)?;
        let server_first = String::from_utf8(server_first).context("SCRAM server-first")?;
        let field = |name: &str| {
            server_first
                .split(',')
                .find_map(|part| part.strip_prefix(name))
                .ok_or_else(|| anyhow!("SCRAM server-first has no `{name}`"))
        };
        let server_nonce = field("r=")?;
        if !server_nonce.starts_with(&nonce) {
            bail!("SCRAM server nonce does not extend the client's");
        }
        let salt = base64_decode(field("s=")?)?;
        let iterations: u32 = field("i=")?.parse().context("SCRAM iteration count")?;

        let salted = scram_salted_password(password, &salt, iterations);
        let client_key = hmac_sha256(&salted, b"Client Key");
        let stored_key = Sha256::digest(client_key);
        let without_proof = format!("c=biws,r={server_nonce}");
        let auth_message = format!("{client_first_bare},{server_first},{without_proof}");
        let signature = hmac_sha256(&stored_key, auth_message.as_bytes());
        let proof: Vec<u8> = client_key
            .iter()
            .zip(signature.iter())
            .map(|(key, sig)| key ^ sig)
            .collect();
        let client_final = format!("{without_proof},p={}", base64_encode(&proof));
        self.send(b'p', client_final.as_bytes())?;

        let server_final = self.sasl_step(12)?;
        let server_key = hmac_sha256(&salted, b"Server Key");
        let expected = format!(
            "v={}",
            base64_encode(&hmac_sha256(&server_key, auth_message.as_bytes()))
        );
        if server_final != expected.as_bytes() {
            bail!("SCRAM server signature does not match");
        }
        Ok(())
    }

    /// The data of the next SASL authentication message of type `code`.
    fn sasl_step(&mut self, code: u32) -> Result<Vec<u8>> {
        let message = self.read_message()?;
        match message.tag {
            b'R' if be_u32(&message.body, 0)? == code => Ok(message.body[4..].to_vec()),
            b'E' => bail!(
                "PostgreSQL refused the login: {}",
                error_text(&message.body)
            ),
            tag => bail!("unexpected PostgreSQL message `{}` in SCRAM", tag as char),
        }
    }

    /// Runs `sql` (simple query protocol) and hands every row to `each`, its
    /// columns in text format, `None` for NULL.
    pub fn query(
        &mut self,
        sql: &str,
        each: &mut dyn FnMut(Vec<Option<Vec<u8>>>) -> Result<()>,
    ) -> Result<()> {
        let mut body = Vec::new();
        put_cstr(&mut body, sql);
        self.send(b'Q', &body)?;
        let mut failure = None;
        loop {
            let message = self.read_message()?;
            match message.tag {
                b'D' if failure.is_none() => {
                    if let Err(error) = each(data_row(&message.body)?) {
                        failure = Some(error);
                    }
                }
                b'E' => {
                    failure.get_or_insert_with(|| {
                        anyhow!("PostgreSQL query failed: {}", error_text(&message.body))
                    });
                }
                b'Z' => return failure.map_or(Ok(()), Err),
                // RowDescription, CommandComplete, EmptyQueryResponse,
                // NoticeResponse, ParameterStatus; a DataRow after a failure.
                _ => {}
            }
        }
    }

    /// Send a single binary COPY statement. On a producer error, abort COPY and
    /// consume ReadyForQuery before returning control to the transaction owner.
    /// This synchronous writer reads server notices at completion. Use it for
    /// staging tables without triggers that emit messages for each input row.
    pub fn copy_binary(
        &mut self,
        sql: &str,
        produce: impl FnOnce(&mut BinaryCopy<'_>) -> Result<()>,
    ) -> Result<()> {
        let mut command = Vec::new();
        put_cstr(&mut command, sql);
        self.send(b'Q', &command)?;
        let response = loop {
            let message = self.read_message()?;
            match message.tag {
                b'G' => break message.body,
                b'E' => {
                    self.finish_copy()?;
                    bail!("PostgreSQL COPY failed: {}", error_text(&message.body));
                }
                b'Z' => bail!("expected PostgreSQL CopyInResponse"),
                _ => continue,
            }
        };
        let sent = (|| {
            let count = response.get(1..3).context("short CopyInResponse")?;
            let columns = u16::from_be_bytes(count.try_into()?);
            if response[0] != 1
                || response.len() != 3 + usize::from(columns) * 2
                || !response[3..].chunks_exact(2).all(|format| format == [0, 1])
            {
                bail!("expected binary PostgreSQL COPY columns");
            }
            self.send(b'd', b"PGCOPY\n\xff\r\n\0\0\0\0\0\0\0\0\0")?;
            produce(&mut BinaryCopy {
                connection: self,
                columns,
                buffer: Vec::new(),
            })?;
            self.send(b'd', &(-1i16).to_be_bytes())?;
            self.send(b'c', &[])
        })();
        if let Err(error) = sent {
            self.send(b'f', b"client aborted COPY\0")?;
            let _ = self.finish_copy();
            return Err(error);
        }
        self.finish_copy()
    }

    fn finish_copy(&mut self) -> Result<()> {
        let mut failure = None;
        loop {
            let message = self.read_message()?;
            match message.tag {
                b'E' => {
                    failure = Some(anyhow!(
                        "PostgreSQL COPY failed: {}",
                        error_text(&message.body)
                    ))
                }
                b'Z' => return failure.map_or(Ok(()), Err),
                _ => {}
            }
        }
    }

    fn send(&mut self, tag: u8, body: &[u8]) -> Result<()> {
        let length = i32::try_from(body.len())
            .ok()
            .and_then(|n| n.checked_add(4))
            .context("PostgreSQL message too large")?;
        let mut header = [tag; 5];
        header[1..].copy_from_slice(&length.to_be_bytes());
        self.stream.get_mut().write_all(&header)?;
        self.stream.get_mut().write_all(body)?;
        self.stream.get_mut().flush()?;
        Ok(())
    }

    fn read_message(&mut self) -> Result<Message> {
        let mut head = [0u8; 5];
        self.stream
            .read_exact(&mut head)
            .context("PostgreSQL closed the connection")?;
        let length = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
        if length < 4 {
            bail!("malformed PostgreSQL message length {length}");
        }
        let mut body = vec![0u8; length - 4];
        self.stream.read_exact(&mut body)?;
        Ok(Message { tag: head[0], body })
    }
}

/// Row encoder for a server-accepted binary COPY.
pub struct BinaryCopy<'a> {
    connection: &'a mut PgConnection,
    columns: u16,
    buffer: Vec<u8>,
}

impl BinaryCopy<'_> {
    /// Fields must already use the PostgreSQL binary format of their column.
    pub fn row(&mut self, fields: &[Option<&[u8]>]) -> Result<()> {
        if fields.len() != usize::from(self.columns) {
            bail!("wrong number of PostgreSQL COPY fields");
        }
        self.buffer.clear();
        self.buffer.extend_from_slice(&self.columns.to_be_bytes());
        for field in fields {
            let length = match field {
                Some(bytes) => i32::try_from(bytes.len()).context("COPY field too large")?,
                None => -1,
            };
            self.buffer.extend_from_slice(&length.to_be_bytes());
            if let Some(bytes) = field {
                self.buffer.extend_from_slice(bytes);
            }
        }
        self.connection.send(b'd', &self.buffer)
    }
}

impl Drop for PgConnection {
    fn drop(&mut self) {
        // Terminate.
        let _ = self.stream.get_mut().write_all(&[b'X', 0, 0, 0, 4]);
        let _ = self.stream.get_mut().flush();
    }
}

fn data_row(body: &[u8]) -> Result<Vec<Option<Vec<u8>>>> {
    let count = u16::from_be_bytes([
        *body.first().context("empty DataRow")?,
        *body.get(1).context("short DataRow")?,
    ]) as usize;
    let mut at = 2;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let length = be_u32(body, at)? as i32;
        at += 4;
        if length < 0 {
            values.push(None);
            continue;
        }
        let end = at + length as usize;
        values.push(Some(
            body.get(at..end)
                .context("DataRow value beyond its message")?
                .to_vec(),
        ));
        at = end;
    }
    Ok(values)
}

/// The text of an ErrorResponse: severity, SQLSTATE and message.
fn error_text(body: &[u8]) -> String {
    let mut severity = "";
    let mut code = "";
    let mut message = "";
    for field in body.split(|byte| *byte == 0) {
        let Some((&kind, value)) = field.split_first() else {
            continue;
        };
        let value = std::str::from_utf8(value).unwrap_or("");
        match kind {
            b'S' => severity = value,
            b'C' => code = value,
            b'M' => message = value,
            _ => {}
        }
    }
    format!("{severity} {code}: {message}")
}

/// A bytea value as hex digits: `encode(value, 'hex')`, or the column in
/// text format under `bytea_output = hex` (`\x` first).
pub fn decode_bytea(text: &[u8]) -> Result<Vec<u8>> {
    let digits = text.strip_prefix(b"\\x").unwrap_or(text);
    if !digits.len().is_multiple_of(2) {
        bail!("bytea hex value of odd length");
    }
    let nibble = |byte: u8| -> Result<u8> {
        Ok(match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => bail!("bytea value holds a non-hex digit"),
        })
    };
    digits
        .chunks_exact(2)
        .map(|pair| Ok((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

fn put_cstr(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(0);
}

fn be_u32(body: &[u8], at: usize) -> Result<u32> {
    let bytes = body.get(at..at + 4).context("short PostgreSQL message")?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|byte| byte ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(block.map(|byte| byte ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut first = salt.to_vec();
    first.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &first);
    let mut out = u;
    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for (acc, byte) in out.iter_mut().zip(u.iter()) {
            *acc ^= byte;
        }
    }
    out
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(BASE64[(n >> (18 - 6 * index) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = BASE64
            .iter()
            .position(|known| *known == byte)
            .ok_or_else(|| anyhow!("bad base64 in SCRAM"))? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Ok(out)
}

fn scram_salted_password(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    // PostgreSQL uses the original bytes when SASLprep rejects a password.
    // See its SASL authentication contract, rather than rejecting such users.
    let prepared = stringprep::saslprep(password).unwrap_or_else(|_| password.into());
    pbkdf2_sha256(prepared.as_bytes(), salt, iterations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_server_the_way_1c_names_it() {
        let target = |server: &str| PgTarget::new(server, "db", None, None).unwrap();
        assert_eq!(
            (target("srv").host, target("srv").port),
            ("srv".into(), 5432)
        );
        assert_eq!(target("srv:5433").port, 5433);
        let tls = target("srv:5433; sslmode=verify-full");
        assert_eq!(
            (tls.host.as_str(), tls.port, tls.verify_full),
            ("srv", 5433, true)
        );
        assert_eq!(target("srv; sslmode=verify-full").host, "srv");
        let spaced = target("srv port=5434;");
        assert!(target("srv port=5434; sslmode=verify-full").verify_full);
        assert!(PgTarget::new("srv sslmode=prefer", "db", None, None).is_err());
        assert_eq!((spaced.host.as_str(), spaced.port), ("srv", 5434));
        assert_eq!(target("srv").user, "postgres");
        assert!(PgTarget::new("srv other", "db", None, None).is_err());
    }

    #[test]
    fn delivers_the_first_row_before_the_server_completes_the_query() {
        use std::net::TcpListener;
        use std::sync::mpsc;

        fn read_frame(socket: &mut TcpStream) -> Vec<u8> {
            let mut length = [0; 4];
            socket.read_exact(&mut length).unwrap();
            let mut body = vec![0; u32::from_be_bytes(length) as usize - 4];
            socket.read_exact(&mut body).unwrap();
            body
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = listener.local_addr().unwrap().to_string();
        let (consumed, gate) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            read_frame(&mut socket);
            socket
                .write_all(b"R\0\0\0\x08\0\0\0\0Z\0\0\0\x05I")
                .unwrap();
            let mut tag = [0];
            socket.read_exact(&mut tag).unwrap();
            assert_eq!(tag, [b'Q']);
            assert_eq!(read_frame(&mut socket), b"SELECT 1\0");
            socket.write_all(b"D\0\0\0\x0b\0\x01\0\0\0\x011").unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            socket
                .write_all(b"C\0\0\0\x0dSELECT 1\0Z\0\0\0\x05I")
                .unwrap();
        });
        let target = PgTarget::new(&server, "synthetic", None, None).unwrap();
        let mut connection = PgConnection::connect(&target).unwrap();
        connection
            .query("SELECT 1", &mut |row| {
                assert_eq!(row, [Some(b"1".to_vec())]);
                consumed.send(())?;
                Ok(())
            })
            .unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn decodes_hex_bytea() {
        assert_eq!(decode_bytea(b"\\x00ff10").unwrap(), vec![0, 255, 16]);
        assert_eq!(decode_bytea(b"00ff").unwrap(), vec![0, 255]);
        assert!(decode_bytea(b"0g").is_err());
        assert!(decode_bytea(b"\\x0").is_err());
    }

    #[test]
    fn scram_password_uses_saslprep_before_hashing() {
        for (password, prepared) in [
            ("I\u{00ad}X", "IX"),
            ("user\u{00a0}name", "user name"),
            ("\u{2168}", "IX"),
        ] {
            assert_eq!(
                scram_salted_password(password, b"synthetic-salt", 2),
                pbkdf2_sha256(prepared.as_bytes(), b"synthetic-salt", 2),
                "password {password:?} must use the PostgreSQL normalization",
            );
        }
    }

    #[test]
    fn scram_prohibited_password_falls_back_to_the_original_bytes() {
        // PostgreSQL deliberately accepts these passwords without SASLprep.
        for password in ["control\u{0007}", "\u{0627}a\u{0628}"] {
            assert_eq!(
                scram_salted_password(password, b"synthetic-salt", 2),
                pbkdf2_sha256(password.as_bytes(), b"synthetic-salt", 2),
            );
        }
    }

    #[test]
    fn scram_primitives_match_rfc_7677() {
        // RFC 7677, section 3: user "user", password "pencil".
        let salt = base64_decode("W22ZaJ0SNY7soEsUEjb6gQ==").unwrap();
        let salted = pbkdf2_sha256(b"pencil", &salt, 4096);
        let client_key = hmac_sha256(&salted, b"Client Key");
        let stored_key = Sha256::digest(client_key);
        let auth_message = "n=user,r=rOprNGfwEbeRWgbNEkqO,\
                            r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
                            s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096,\
                            c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0";
        let signature = hmac_sha256(&stored_key, auth_message.as_bytes());
        let proof: Vec<u8> = client_key
            .iter()
            .zip(signature.iter())
            .map(|(key, sig)| key ^ sig)
            .collect();
        assert_eq!(
            base64_encode(&proof),
            "dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        let server_key = hmac_sha256(&salted, b"Server Key");
        assert_eq!(
            base64_encode(&hmac_sha256(&server_key, auth_message.as_bytes())),
            "6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="
        );
    }

    #[test]
    fn base64_round_trips() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob"] {
            assert_eq!(base64_decode(&base64_encode(bytes)).unwrap(), bytes);
        }
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    }
}
