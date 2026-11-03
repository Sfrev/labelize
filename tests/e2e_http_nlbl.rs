//! HTTP e2e for POST /nlbl over a real `labelize serve` process: a design
//! file in, its measurements as JSON out, and the returned `convert_query`
//! rendering the exported ZPL on the printer's exact dots. Requires the
//! `serve` feature (CI runs it beside `e2e_http_encode`).
//!
//! Archives are synthesised and encrypted with a throwaway password; every
//! server runs from an empty directory so a developer's real `./.env` never
//! leaks into the test.

#![cfg(feature = "serve")]

#[path = "common/nlbl_fixtures.rs"]
mod nlbl_fixtures;
use nlbl_fixtures::{archive, format_xml, with_radius, zd621_300dpi, TEST_PASSWORD};

struct ServerGuard {
    child: std::process::Child,
    _cwd: TempDir,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A scratch working directory, removed on drop.
struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind for port probe")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Start `labelize serve` with `password` as LABELIZE_NLBL_PASSWORD (None =
/// unset) from an empty working directory.
fn start_server(password: Option<&str>) -> (ServerGuard, reqwest::blocking::Client, String) {
    let port = free_port();
    let cwd = std::env::temp_dir().join(format!("labelize-nlbl-http-{port}"));
    std::fs::create_dir_all(&cwd).expect("scratch cwd");
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_labelize"));
    cmd.args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
        .current_dir(&cwd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match password {
        Some(pw) => cmd.env("LABELIZE_NLBL_PASSWORD", pw),
        None => cmd.env_remove("LABELIZE_NLBL_PASSWORD"),
    };
    let child = cmd
        .spawn()
        .expect("spawn labelize serve (requires --features serve)");
    let guard = ServerGuard {
        child,
        _cwd: TempDir(cwd),
    };
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("http client");
    for _ in 0..150 {
        if let Ok(resp) = client.get(format!("{base}/health")).send() {
            if resp.status().is_success() {
                return (guard, client, base);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("labelize serve did not become healthy on {base}");
}

/// The 60 x 30 mm label with 3 mm margins and 1 mm corners for a 300-dpi
/// ZD621.
fn zd621_label(password: Option<&'static str>) -> Vec<u8> {
    let xml = format_xml(
        (66000, 36000),
        (60000, 30000),
        (3000, 3000),
        &zd621_300dpi(),
    );
    archive(&with_radius(&xml, 1000, 1000), password)
}

#[test]
fn nlbl_reports_measurements_and_a_dot_exact_convert_query() {
    let (_guard, client, base) = start_server(Some(TEST_PASSWORD));

    let resp = client
        .post(format!("{base}/nlbl"))
        .body(zd621_label(Some(TEST_PASSWORD)))
        .send()
        .expect("POST /nlbl");
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()["content-type"].to_str().unwrap(),
        "application/json"
    );
    let json: serde_json::Value = serde_json::from_str(&resp.text().unwrap()).expect("json body");
    assert_eq!(json["label_width_mm"], 60.0);
    assert_eq!(json["label_height_mm"], 30.0);
    assert_eq!(json["page_width_mm"], 66.0);
    assert_eq!(json["page_height_mm"], 36.0);
    assert_eq!(json["margins_mm"]["left"], 3.0);
    assert_eq!(json["radius_mm"]["horizontal"], 1.0);
    assert_eq!(json["radius_mm"]["vertical"], 1.0);
    assert_eq!(json["dpi"], 300);
    assert_eq!(json["dpmm"], 12);
    assert_eq!(json["printer"], "ZDesigner ZD621-300dpi ZPL");

    // The query renders the designer's ^PW780/^LL425 page dot for dot, with
    // the colour guide overlay on.
    let query = json["convert_query"].as_str().expect("convert_query");
    assert!(query.contains("&radius="), "{query}");
    let png = client
        .post(format!("{base}/convert?{query}"))
        .header("Content-Type", "application/zpl")
        .body("^XA^PW780^LL425^FO100,100^GB50,50,50^FS^XZ")
        .send()
        .expect("POST /convert")
        .bytes()
        .unwrap();
    let img = image::load_from_memory(&png).expect("png");
    assert_eq!((img.width(), img.height()), (780, 425));
    assert_eq!(img.color(), image::ColorType::Rgb8, "guides are on");
}

#[test]
fn plain_archives_open_even_with_a_server_password() {
    let (_guard, client, base) = start_server(Some(TEST_PASSWORD));
    let resp = client
        .post(format!("{base}/nlbl"))
        .body(zd621_label(None))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[test]
fn invalid_uploads_are_400() {
    let (_guard, client, base) = start_server(Some(TEST_PASSWORD));
    let resp = client
        .post(format!("{base}/nlbl"))
        .body("^XA^XZ")
        .send()
        .unwrap();
    assert_eq!(resp.status(), 400);
    assert!(resp.text().unwrap().contains("not an .nlbl"));
}

#[test]
fn missing_or_wrong_server_password_is_500_with_a_hint() {
    for password in [None, Some("not-the-password")] {
        let (_guard, client, base) = start_server(password);
        let resp = client
            .post(format!("{base}/nlbl"))
            .body(zd621_label(Some(TEST_PASSWORD)))
            .send()
            .unwrap();
        assert_eq!(resp.status(), 500, "{password:?}");
        let text = resp.text().unwrap();
        assert!(
            text.contains("set LABELIZE_NLBL_PASSWORD on the server"),
            "{text}"
        );
    }
}
