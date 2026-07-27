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
#[path = "../tests/http_request.rs"]
mod tests;
