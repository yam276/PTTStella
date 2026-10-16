//! Fixed, application-owned font assets; never resolves arbitrary local paths.
use dioxus::desktop::wry::http::{Method, Request, Response, StatusCode, header};
use std::borrow::Cow;

pub fn response(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed(b"".as_slice()));
    if request.method() != Method::GET && request.method() != Method::HEAD {
        *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        response
            .headers_mut()
            .insert(header::ALLOW, header::HeaderValue::from_static("GET, HEAD"));
        return response;
    }
    let (bytes, mime): (&'static [u8], &'static str) = match request.uri().path() {
        "/pttstella-fonts/CascadiaMono.ttf" => (
            include_bytes!("../assets/fonts/CascadiaMono.ttf"),
            "font/ttf",
        ),
        "/pttstella-fonts/NotoSansCJKtc-Regular.otf" => (
            include_bytes!("../assets/fonts/NotoSansCJKtc-Regular.otf"),
            "font/otf",
        ),
        _ => {
            *response.status_mut() = StatusCode::NOT_FOUND;
            return response;
        }
    };
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, header::HeaderValue::from_static(mime));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if request.method() == Method::GET {
        *response.body_mut() = Cow::Borrowed(bytes);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_font_routes_have_binary_types_and_no_arbitrary_file_access() {
        for (path, mime, magic) in [
            (
                "/pttstella-fonts/CascadiaMono.ttf",
                "font/ttf",
                b"\x00\x01\x00\x00".as_slice(),
            ),
            (
                "/pttstella-fonts/NotoSansCJKtc-Regular.otf",
                "font/otf",
                b"OTTO",
            ),
        ] {
            let request = Request::builder().uri(path).body(Vec::new()).unwrap();
            let result = response(&request);
            assert_eq!(result.status(), StatusCode::OK);
            assert_eq!(result.headers()[header::CONTENT_TYPE], mime);
            assert!(result.body().starts_with(magic));
            assert!((512_000..20_000_000).contains(&result.body().len()));
            let request = Request::builder()
                .method(Method::HEAD)
                .uri(path)
                .body(Vec::new())
                .unwrap();
            assert!(response(&request).body().is_empty());
        }
        for path in [
            "/pttstella-fonts/../Cargo.toml",
            "/pttstella-fonts/%2e%2e/Cargo.toml",
            "/pttstella-fonts/MicrosoftJhengHei.ttf",
        ] {
            let request = Request::builder().uri(path).body(Vec::new()).unwrap();
            let result = response(&request);
            assert_eq!(result.status(), StatusCode::NOT_FOUND);
            assert!(result.body().is_empty());
        }
        let request = Request::builder()
            .method(Method::POST)
            .uri("/pttstella-fonts/CascadiaMono.ttf")
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(&request).status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
