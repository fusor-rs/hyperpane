//! A small HTTP server inside the host, on this device's own 127.0.0.1, for
//! fusor applications whose worker pools share memory.
//!
//! WebKit gives a page `SharedArrayBuffer` only when the page comes from an
//! `http://` origin whose responses send the cross-origin isolation headers;
//! on the `hyperpane://` scheme it stays undefined, on macOS and on iOS. fusor
//! marks such applications with `.fusor-worker-headers.json`, and for them
//! the host loads `http://127.0.0.1:<port>/` from this server instead of the
//! scheme. Every other application never starts it.
//!
//! The server decides nothing about files: each request goes to
//! [`Publication::respond`], the same answer the scheme gives, which also adds
//! the isolation headers. Only this device can connect, and it runs for as
//! long as the host does.
use crate::{
    Error, ErrorKind,
    publication::{Asset, Publication},
};
use http::{Method, Request};
use std::{
    io::{self, BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::Arc,
    thread,
};

/// Listens on 127.0.0.1 at a port the system picks, serves `publication` in
/// the background and returns the address to load.
pub(crate) fn start(publication: Publication) -> Result<SocketAddr, Error> {
    let unavailable = |error: io::Error| {
        Error::new(
            ErrorKind::Loopback,
            format!("cannot listen on the loopback interface: {error}"),
            "allow local network listeners for this app, or build the application without a shared-memory worker pool",
        )
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(unavailable)?;
    let address = listener.local_addr().map_err(unavailable)?;
    let publication = Arc::new(publication);
    thread::spawn(move || accept_connections(&listener, &publication));
    Ok(address)
}

/// One thread per connection: WebKit opens several at once, and one slow
/// request must not hold up the others.
fn accept_connections(listener: &TcpListener, publication: &Arc<Publication>) {
    for connection in listener.incoming() {
        let publication = Arc::clone(publication);
        thread::spawn(move || {
            if let Err(error) =
                connection.and_then(|stream| answer_one_request(&stream, &publication))
            {
                eprintln!("hyperpane-host: a loopback request failed: {error}");
            }
        });
    }
}

/// Each connection carries one request and is then closed, so the server
/// never has to track keep-alive connections.
fn answer_one_request(stream: &TcpStream, publication: &Publication) -> io::Result<()> {
    let request = read_request(stream)?;
    write_response(stream, request.method(), &publication.respond(&request))
}

fn read_request(stream: &TcpStream) -> io::Result<Request<()>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut request_line = line.split_whitespace();
    let (Some(method), Some(target)) = (request_line.next(), request_line.next()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "malformed request line",
        ));
    };
    let mut request = Request::builder().method(method).uri(target);
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let Some((name, value)) = line.split_once(':') else {
            break;
        };
        request = request.header(name.trim(), value.trim());
    }
    request.body(()).map_err(io::Error::other)
}

fn write_response(mut stream: &TcpStream, method: &Method, response: &Asset) -> io::Result<()> {
    let status = response.status();
    let mut head = Vec::new();
    write!(
        head,
        "HTTP/1.1 {} {}\r\n",
        status.as_str(),
        status.canonical_reason().unwrap_or_default()
    )?;
    for (name, value) in response.headers() {
        head.extend_from_slice(name.as_str().as_bytes());
        head.extend_from_slice(b": ");
        head.extend_from_slice(value.as_bytes());
        head.extend_from_slice(b"\r\n");
    }
    write!(
        head,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body().len()
    )?;
    stream.write_all(&head)?;
    if method != Method::HEAD {
        stream.write_all(response.body())?;
    }
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::start;
    use crate::publication::{
        Origin, Publication,
        tests::{MANIFEST, publish},
    };
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };

    #[test]
    fn serves_shared_memory_publications_with_isolation_headers() {
        let root = tempfile::tempdir().unwrap();
        publish(
            root.path(),
            &[
                (".fusor-output.json", MANIFEST),
                (".fusor-worker-headers.json", "{}"),
                ("index.html", "home"),
            ],
        );
        let publication = Publication::open(root.path()).unwrap();
        assert_eq!(publication.origin(), Origin::Loopback);
        let address = start(publication).unwrap();
        assert!(address.ip().is_loopback());
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /details HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/html\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\n\
             content-type: text/html; charset=utf-8\r\n\
             cross-origin-opener-policy: same-origin\r\n\
             cross-origin-embedder-policy: require-corp\r\n\
             Content-Length: 4\r\n\
             Connection: close\r\n\
             \r\n\
             home"
        );
    }
}
