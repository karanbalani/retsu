use actix_web::http::Version;

pub(super) fn normalized_method(method: &str) -> &str {
    match method {
        "CONNECT" | "DELETE" | "GET" | "HEAD" | "OPTIONS" | "PATCH" | "POST" | "PUT" | "QUERY"
        | "TRACE" => method,
        _ => "_OTHER",
    }
}

pub(super) fn normalized_scheme(scheme: &str) -> &str {
    match scheme {
        "http" | "https" => scheme,
        _ => "_OTHER",
    }
}

pub(super) fn protocol_version(version: Version) -> Option<&'static str> {
    match version {
        Version::HTTP_09 => Some("0.9"),
        Version::HTTP_10 => Some("1.0"),
        Version::HTTP_11 => Some("1.1"),
        Version::HTTP_2 => Some("2"),
        Version::HTTP_3 => Some("3"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::Version;

    use super::{normalized_method, normalized_scheme, protocol_version};

    #[test]
    fn preserves_standard_http_methods() {
        let methods = [
            "CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "QUERY", "TRACE",
        ];

        for method in methods {
            assert_eq!(normalized_method(method), method);
        }
    }

    #[test]
    fn bounds_unknown_http_methods() {
        assert_eq!(normalized_method("PURGE"), "_OTHER");
        assert_eq!(normalized_method("get"), "_OTHER");
        assert_eq!(normalized_method(""), "_OTHER");
    }

    #[test]
    fn preserves_only_known_url_schemes() {
        assert_eq!(normalized_scheme("http"), "http");
        assert_eq!(normalized_scheme("https"), "https");
        assert_eq!(normalized_scheme("ftp"), "_OTHER");
        assert_eq!(normalized_scheme("HTTP"), "_OTHER");
    }

    #[test]
    fn maps_supported_protocol_versions() {
        let cases = [
            (Version::HTTP_09, "0.9"),
            (Version::HTTP_10, "1.0"),
            (Version::HTTP_11, "1.1"),
            (Version::HTTP_2, "2"),
            (Version::HTTP_3, "3"),
        ];

        for (version, expected) in cases {
            assert_eq!(protocol_version(version), Some(expected));
        }
    }
}
