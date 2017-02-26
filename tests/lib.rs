extern crate mockito;

use std::net::TcpStream;
use std::io::{Read, Write, BufRead, BufReader};
use mockito::{SERVER_ADDRESS, mock, reset};

fn request_stream(route: &str, headers: &str) -> TcpStream {
    let mut stream = TcpStream::connect(SERVER_ADDRESS).unwrap();
    let message = [route, " HTTP/1.1\r\n", headers, "\r\n"].join("");
    stream.write_all(message.as_bytes()).unwrap();

    stream
}

fn parse_stream(stream: TcpStream, content_length: usize) -> (String, Vec<String>, String) {
    let mut reader = BufReader::new(stream);

    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();

    let mut headers = vec![];
    loop {
        let mut header_line = String::new();
        reader.read_line(&mut header_line).unwrap();

        if header_line == "\r\n" { break }
        else { headers.push(header_line); }
    }

    let mut body = String::new();
    reader.take(content_length as u64).read_to_string(&mut body).unwrap();

    (status_line, headers, body)
}

fn request(route: &str, headers: &str, expected_content_length: usize) -> (String, Vec<String>, String) {
    parse_stream(request_stream(route, headers), expected_content_length)
}

#[test]
fn test_create_starts_the_server() {
    mock("GET", "/").with_body("hello").create();

    let stream = TcpStream::connect(SERVER_ADDRESS);
    assert!(stream.is_ok());
}

#[test]
fn test_simple_route_mock() {
    reset();

    let mocked_body = "world";
    mock("GET", "/hello").with_body(mocked_body).create();

    let (status_line, _, body) = request("GET /hello", "", 5);
    assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", status_line);
    assert_eq!(mocked_body, body);
}

#[test]
fn test_two_route_mocks() {
    reset();

    mock("GET", "/a").with_body("aaa").create();
    mock("GET", "/b").with_body("bbb").create();

    let (_, _, body_a) = request("GET /a", "", 3);

    assert_eq!("aaa", body_a);
    let (_, _, body_b) = request("GET /b", "", 3);
    assert_eq!("bbb", body_b);
}

#[test]
fn test_no_match_returns_501() {
    reset();

    mock("GET", "/").with_body("matched").create();

    let (status_line, _, _) = request("GET /nope", "", 0);
    assert_eq!("HTTP/1.1 501 Not Implemented\r\n", status_line);
}

#[test]
fn test_match_header() {
    reset();

    mock("GET", "/")
        .match_header("content-type", "application/json")
        .with_body("{}")
        .create();

    mock("GET", "/")
        .match_header("content-type", "text/plain")
        .with_body("hello")
        .create();

    let (_, _, body_json) = request("GET /", "content-type: application/json\r\n", 2);
    assert_eq!("{}", body_json);

    let (_, _, body_text) = request("GET /", "content-type: text/plain\r\n", 5);
    assert_eq!("hello", body_text);
}

#[test]
fn test_match_header_is_case_insensitive_on_the_field_name() {
    reset();

    mock("GET", "/").match_header("content-type", "text/plain").create();

    let (uppercase_status_line, _, _) = request("GET /", "Content-Type: text/plain\r\n", 0);
    assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", uppercase_status_line);

    let (lowercase_status_line, _, _) = request("GET /", "content-type: text/plain\r\n", 0);
    assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", lowercase_status_line);
}

#[test]
fn test_match_multiple_headers() {
    reset();

    mock("GET", "/")
        .match_header("Content-Type", "text/plain")
        .match_header("Authorization", "secret")
        .with_body("matched")
        .create();

    let (_, _, body_matching) = request("GET /", "content-type: text/plain\r\nauthorization: secret\r\n", 7);
    assert_eq!("matched", body_matching);

    let (status_not_matching, _, _) = request("GET /", "content-type: text/plain\r\nauthorization: meh\r\n", 0);
    assert_eq!("HTTP/1.1 501 Not Implemented\r\n", status_not_matching);
}

#[test]
fn test_mock_with_status() {
    reset();

    mock("GET", "/")
        .with_status(204)
        .with_body("")
        .create();

    let (status_line, _, _) = request("GET /", "", 0);
    assert_eq!("HTTP/1.1 204 <unknown status code>\r\n", status_line);
}

#[test]
fn test_mock_with_header() {
    reset();

    mock("GET", "/")
        .with_header("content-type", "application/json")
        .with_body("{}")
        .create();

    let (_, headers, _) = request("GET /", "", 0);
    assert!(headers.contains(&"content-type: application/json\r\n".to_string()));
}

#[test]
fn test_mock_with_multiple_headers() {
    reset();

    mock("GET", "/")
        .with_header("content-type", "application/json")
        .with_header("x-api-key", "1234")
        .with_body("{}")
        .create();

    let (_, headers, _) = request("GET /", "", 0);
    assert!(headers.contains(&"content-type: application/json\r\n".to_string()));
    assert!(headers.contains(&"x-api-key: 1234\r\n".to_string()));
}

#[test]
fn test_reset_clears_mocks() {
    reset();

    mock("GET", "/reset").create();

    let (working_status_line, _, _) = request("GET /reset", "", 0);
    assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", working_status_line);

    reset();

    let (reset_status_line, _, _) = request("GET /reset", "", 0);
    assert_eq!("HTTP/1.1 501 Not Implemented\r\n", reset_status_line);
}

#[test]
fn test_mock_remove_clears_the_mock() {
    reset();

    let mut mock = mock("GET", "/");
    mock.create();

    let (working_status_line, _, _) = request("GET /", "", 0);
    assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", working_status_line);

    mock.remove();

    let (reset_status_line, _, _) = request("GET /", "", 0);
    assert_eq!("HTTP/1.1 501 Not Implemented\r\n", reset_status_line);
}

#[test]
fn test_mock_create_for_is_only_available_during_the_closure_lifetime() {
    reset();

    mock("GET", "/").create_for( || {
        let (working_status_line, _, _) = request("GET /", "", 0);
        assert_eq!("HTTP/1.1 200 <unknown status code>\r\n", working_status_line);
    });

    let (reset_status_line, _, _) = request("GET /", "", 0);
    assert_eq!("HTTP/1.1 501 Not Implemented\r\n", reset_status_line);
}