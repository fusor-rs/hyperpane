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

/// Serves `publication` over HTTP on 127.0.0.1 and an ephemeral port, one
/// thread per connection, for as long as the process lives. WebKit opens
/// several connections at once, so they must not wait on each other.
pub(crate) fn serve(publication: Publication) -> Result<SocketAddr, Error> {
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
    thread::spawn(move || {
        for connection in listener.incoming() {
            let publication = Arc::clone(&publication);
            thread::spawn(move || {
                if let Err(error) = connection.and_then(|stream| answer(&stream, &publication)) {
                    eprintln!("hyperpane-host: a loopback request failed: {error}");
                }
            });
        }
    });
    Ok(address)
}

fn answer(stream: &TcpStream, publication: &Publication) -> io::Result<()> {
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
    use super::serve;
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
        let address = serve(publication).unwrap();
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
