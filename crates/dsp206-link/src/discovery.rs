//! Microchip "announce" discovery on UDP 30303 (protocol.md §1.1).

use std::io::{self, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub const DISCOVERY_PORT: u16 = 30303;
pub const PROBE: &[u8] = b"Discovery: Who is out there?\0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Discovered {
    pub addr: IpAddr,
    /// First line of the reply (the module's host name, e.g. `EHTERNET_COM`).
    pub name: String,
    /// Remaining non-empty lines (partial MAC address, …).
    pub details: Vec<String>,
}

/// Parse one announce reply.
pub fn parse_reply(addr: IpAddr, data: &[u8]) -> Discovered {
    let text = String::from_utf8_lossy(data);
    let mut lines = text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty());
    Discovered {
        addr,
        name: lines.next().unwrap_or_default().to_string(),
        details: lines.map(str::to_string).collect(),
    }
}

/// Broadcast the probe (and send it to `extra` addresses) and collect replies
/// for `wait`.
pub fn discover(wait: Duration, extra: &[IpAddr]) -> io::Result<Vec<Discovered>> {
    let sock = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
    sock.set_broadcast(true)?;
    sock.set_read_timeout(Some(Duration::from_millis(50)))?;
    let mut targets = vec![IpAddr::V4(Ipv4Addr::BROADCAST)];
    targets.extend_from_slice(extra);
    for t in &targets {
        // A failing target (no route) must not stop the others.
        let _ = sock.send_to(PROBE, SocketAddr::new(*t, DISCOVERY_PORT));
    }
    let mut found: Vec<Discovered> = Vec::new();
    let end = Instant::now() + wait;
    let mut buf = [0u8; 512];
    while Instant::now() < end {
        match sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                let d = parse_reply(from.ip(), &buf[..n]);
                if !found.iter().any(|f| f.addr == d.addr) && &buf[..n] != PROBE {
                    found.push(d);
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_captured_announce() {
        // Reply captured from the real module (protocol.md §1.1).
        let ip: IpAddr = "192.168.1.101".parse().unwrap();
        let d = parse_reply(ip, b"EHTERNET_COM   \r\n00-04-A3-C1\r\n1F\r\n");
        assert_eq!(d.addr, ip);
        assert_eq!(d.name, "EHTERNET_COM");
        assert_eq!(d.details, vec!["00-04-A3-C1", "1F"]);
    }
}
