use actix_web::http::Version;

use super::{normalized_method, normalized_scheme, protocol_version};

#[test]
fn normalizes_bounded_http_attributes() {
    let methods = [
        "CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "QUERY", "TRACE",
    ];

    for method in methods {
        assert_eq!(normalized_method(method), method);
    }

    for method in ["PURGE", "get", ""] {
        assert_eq!(normalized_method(method), "_OTHER");
    }

    for (scheme, expected) in [
        ("http", "http"),
        ("https", "https"),
        ("ftp", "_OTHER"),
        ("HTTP", "_OTHER"),
    ] {
        assert_eq!(normalized_scheme(scheme), expected);
    }

    for (version, expected) in [
        (Version::HTTP_09, "0.9"),
        (Version::HTTP_10, "1.0"),
        (Version::HTTP_11, "1.1"),
        (Version::HTTP_2, "2"),
        (Version::HTTP_3, "3"),
    ] {
        assert_eq!(protocol_version(version), Some(expected));
    }
}
