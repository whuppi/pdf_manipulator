//! A timestamp server's reply is read up to a fixed window.

use crate::signatures::HashAlgorithm;
use crate::signatures::{TsaClient, TsaClientConfig};
use std::io::{Read, Write};
use std::net::TcpListener;

/// Serves one HTTP reply of `len` zero bytes and returns the client's error.
fn error_for_reply_of(len: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/tsr", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        // The whole request is read before the reply: a server that closes on
        // unread bytes resets the connection under the reply it just sent.
        let mut request = Vec::new();
        let mut piece = [0u8; 4096];
        let body_at = loop {
            let n = conn.read(&mut piece).unwrap();
            request.extend_from_slice(&piece[..n]);
            if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            assert!(n > 0, "the request ended inside its head");
        };
        let head = String::from_utf8_lossy(&request[..body_at]).to_ascii_lowercase();
        let wanted: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .map(|v| v.trim().parse().unwrap())
            .unwrap_or(0);
        while request.len() < body_at + wanted {
            let n = conn.read(&mut piece).unwrap();
            assert!(n > 0, "the request ended inside its body");
            request.extend_from_slice(&piece[..n]);
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/timestamp-reply\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
        );
        conn.write_all(header.as_bytes()).unwrap();
        let chunk = vec![0u8; 64 * 1024];
        let mut left = len;
        while left > 0 {
            let n = left.min(chunk.len());
            if conn.write_all(&chunk[..n]).is_err() {
                break;
            }
            left -= n;
        }
    });
    let client = TsaClient::new(TsaClientConfig::new(url));
    let err = client
        .request_timestamp_hash(&[0xAB; 32], HashAlgorithm::Sha256)
        .expect_err("a reply of zeroes is no timestamp");
    server.join().unwrap();
    err.to_string()
}

/// A reply of exactly the window is read whole and fails as the bytes it is
/// (not a `TimeStampResp`); one byte more is refused before it is parsed. No
/// Dart battery can observe this: signing reaches a timestamp server only
/// over the network, and the cap is a limit on what the engine reads of a
/// reply, not on what a signature holds.
#[test]
fn a_timestamp_reply_past_the_window_is_an_error() {
    let cap = 1024 * 1024;
    let at_cap = error_for_reply_of(cap);
    assert!(at_cap.contains("not a valid TimeStampResp"), "at the window: {at_cap}");
    let past = error_for_reply_of(cap + 1);
    assert!(past.contains("exceeds"), "one byte past the window: {past}");
}
