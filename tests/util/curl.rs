use std::process::Command;

use my_http::common::request::Request;

pub fn request(url: &str, request: &Request, https: bool) -> String {
    let reqs = [request];
    requests(url, &reqs, https)
}

pub fn repeat_request(url: &str, request: &Request, n: usize, https: bool) -> String {
    requests(url,vec![request; n].as_slice(), https)
}

pub fn requests(url: &str, requests: &[&Request], https: bool) -> String {
    let mut cmd = Command::new("curl");

    if https {
        cmd.arg("-k");
    }

    for request in requests {
        cmd.arg("--request").arg("GET");

        for (name, values) in &request.headers {
            for value in values {
                cmd.arg("--header").arg(format!("\"{}: {}\"", name, value));
            }
        }

        cmd.arg("--data").arg(format!("\"{}\"", String::from_utf8_lossy(&request.body)));

        if https {
            cmd.arg(format!("https://{}{}", url, &request.uri));
        } else {
            cmd.arg(format!("http://{}{}", url, &request.uri));
        }
    }

    String::from_utf8_lossy(&cmd.output().unwrap().stdout).to_string()
}