use std::process::Command;

use my_http::common::request::Request;

pub fn request(addr: &str, request: &Request, https: bool) -> String {
    let reqs = [request];
    requests(addr, &reqs, https)
}

pub fn requests(addr: &str, requests: &[&Request], https: bool) -> String {
    let mut cmd = Command::new("curl");

    for request in requests {
        cmd.arg("--next");

        if https {
            cmd.arg("-k");
        }

        cmd.arg("--request").arg("GET");

        // these are the headers curl includes by default
        // remove them as its better to test them explicitly
        cmd.arg("-H").arg("Content-Type:");
        cmd.arg("-H").arg("Host:");
        cmd.arg("-H").arg("User-Agent:");
        cmd.arg("-H").arg("Accept:");
        cmd.arg("-H").arg("Referer:");

        for (name, values) in &request.headers {
            for value in values {
                cmd.arg("--header").arg(format!("{}: {}", name, value));
            }
        }

        if !request.body.is_empty() {
            cmd.arg("--data").arg(format!("{}", String::from_utf8_lossy(&request.body)));
        }

        if https {
            cmd.arg(format!("https://{}{}", addr, &request.uri));
        } else {
            cmd.arg(format!("http://{}{}", addr, &request.uri));
        }
    }

    String::from_utf8_lossy(&cmd.output().unwrap().stdout).to_string()
}