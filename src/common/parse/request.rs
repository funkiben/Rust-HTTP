use std::io::BufRead;

use crate::common::HTTP_VERSION;
use crate::common::method::Method;
use crate::common::parse::common::{ParsingError, read_message};
use crate::common::request::Request;

/// The possible errors that can be encountered when trying to parse a request.
#[derive(Debug)]
pub enum RequestParsingError {
    /// Method is unrecognized.
    UnrecognizedMethod(String),
    /// Base parsing error.
    Base(ParsingError),
}

impl From<ParsingError> for RequestParsingError {
    fn from(err: ParsingError) -> Self {
        RequestParsingError::Base(err)
    }
}

/// Reads a request from the given buffered reader.
/// If the data from the reader does not form a valid request or the connection has been closed, returns an error.
pub fn read_request(reader: &mut impl BufRead) -> Result<Request, RequestParsingError> {
    let (first_line, headers, body) = read_message(reader, false)?;

    let (method, uri, http_version) = parse_first_line(&first_line)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ParsingError::WrongHttpVersion.into());
    }

    Ok(Request { method, uri: uri.to_string(), headers, body })
}


/// Parses the given line as the first line of a request.
/// The first lines of requests have the form: "Method Request-URI HTTP-Version CRLF"
fn parse_first_line(line: &str) -> Result<(Method, &str, &str), RequestParsingError> {
    let mut split = line.split(" ");

    let method_raw = split.next().ok_or(ParsingError::BadSyntax)?;
    let uri = split.next().ok_or(ParsingError::BadSyntax)?;
    let http_version = split.next().ok_or(ParsingError::BadSyntax)?;

    Ok((parse_method(method_raw)?, uri, http_version))
}

/// Parses the given string into a method. If the method is not recognized, will return an error.
fn parse_method(raw: &str) -> Result<Method, RequestParsingError> {
    // TODO move this to method module
    match raw {
        "GET" => Ok(Method::GET),
        "POST" => Ok(Method::POST),
        "DELETE" => Ok(Method::DELETE),
        "PUT" => Ok(Method::PUT),
        _ => Err(RequestParsingError::UnrecognizedMethod(String::from(raw)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::common::header::{HeaderMap, CONTENT_LENGTH, CONNECTION};
    use crate::common::method::Method;
    use crate::common::parse::{read_request, RequestParsingError};
    use crate::common::parse::common::ParsingError::{BadSyntax, EOF, InvalidHeaderValue, WrongHttpVersion, Reading, UnexpectedEOF};
    use crate::common::parse::RequestParsingError::UnrecognizedMethod;
    use crate::common::request::Request;
    use crate::util::mock::MockReader;
    use crate::header_map;

    fn test_read_request(data: Vec<&str>, expected_result: Result<Request, RequestParsingError>) {
        let reader = MockReader::from(data);
        let mut reader = BufReader::new(reader);
        let actual_result = read_request(&mut reader);
        match (expected_result, actual_result) {
            (Ok(exp), Ok(act)) => assert_eq!(exp, act),
            (exp, act) => assert_eq!(format!("{:?}", exp), format!("{:?}", act))
        }
    }

    #[test]
    fn no_data() {
        test_read_request(vec![], Err(EOF.into()));
    }

    #[test]
    fn no_header_or_body() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn no_header_or_body_fragmented() {
        test_read_request(
            vec!["G", "ET / ", "HTTP/1", ".1\r\n", "\r", "\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn interesting_uri() {
        test_read_request(
            vec!["GET /hello/world/ HTTP/1.1\r\n\r\n"],
            Ok(Request {
                uri: String::from("/hello/world/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn weird_uri() {
        test_read_request(
            vec!["GET !#%$#/-+=_$+[]{}\\%&$ HTTP/1.1\r\n\r\n"],
            Ok(Request {
                uri: String::from("!#%$#/-+=_$+[]{}\\%&$"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn many_spaces_in_first_line() {
        test_read_request(
            vec!["GET /hello/world/ HTTP/1.1 hello there blah blah\r\n\r\n"],
            Ok(Request {
                uri: String::from("/hello/world/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn only_reads_one_request() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn headers() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\nconnection: close\r\nsomething: hello there goodbye\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "0"),
                    (CONNECTION, "close"),
                    ("something", "hello there goodbye"),
                ],
                body: vec![],
            }))
    }

    #[test]
    fn repeated_headers() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\ncontent-length: 0\r\nsomething: value 1\r\nsomething: value 2\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "0"),
                    (CONTENT_LENGTH, "0"),
                    ("something", "value 1"),
                    ("something", "value 2"),
                ],
                body: vec![],
            }))
    }

    #[test]
    fn headers_weird_case() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncoNtEnt-lEngtH: 0\r\nCoNNECTION: close\r\nsoMetHing: hello there goodbye\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "0"),
                    (CONNECTION, "close"),
                    ("something", "hello there goodbye"),
                ],
                body: vec![],
            }))
    }

    #[test]
    fn headers_only_colon_and_space() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\n: \r\n: \r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    ("", ""),
                    ("", ""),
                ],
                body: vec![],
            }))
    }

    #[test]
    fn body_with_content_length() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "5"),
                ],
                body: b"hello".to_vec(),
            }))
    }

    #[test]
    fn body_fragmented() {
        test_read_request(
            vec!["GE", "T / ", "HTT", "P/1.", "1\r", "\nconte", "nt-le", "n", "gth: ", "5\r\n\r", "\nhe", "ll", "o"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "5"),
                ],
                body: b"hello".to_vec(),
            }))
    }

    #[test]
    fn two_requests_with_bodies() {
        test_read_request(
            vec![
                "GET /body1 HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello",
                "GET /body2 HTTP/1.1\r\ncontent-length: 7\r\n\r\ngoodbye"
            ],
            Ok(
                Request {
                    uri: String::from("/body1"),
                    method: Method::GET,
                    headers: header_map![
                        (CONTENT_LENGTH, "5"),
                    ],
                    body: b"hello".to_vec(),
                }),
        )
    }

    #[test]
    fn large_body() {
        let body = b"ergiergjhlisuehrlgisuehrlgisuehrlgiushelrgiushelriguheisurhgl ise\
        uhrg laiuwe````hrg ;aoiwhg aw4tyg 8o3w74go 8w475g\no 8w475hgo 8w475hgo 84w75hgo 8w347hfo g83qw7h4go\
         q837hgp 9q384h~~~gp 9qw\r\n385hgp q9384htpq9 38\r\nwuhf iwourehafgliweurhglaieruhgq9w348gh q9384ufhq\
         uerhgfq 934g\\hq934h|][;[.',,/.fg 9w`234145365uerhfg iawo!@#$$%#^$%&^$%^(&*^)(_)+_){P.;o\\/]'o;\n\n\r\n
         \r\n\n\r\n\r]/li][.                                                                       \
         \n\n\n\n\n\n\n\n\n     ^$%@#%!@%!@$%@#$^&%*&&^&()&)(|>wiuerghwiefujwouegowogjoe rijgoe rg\
         eriopgjeorgj eorgij woergij owgj 9348t9 348uqwtp 3874hg ow3489ghqp 9348ghf qp3498ugh pq\
         3q489g pq3498gf qp3948fh qp39ruhgwirughp9q34ughpq34u9gh pq3g\
         3q498g7 hq3o84g7h q3o847gh qp3948fh pq9wufhp q9w4hgpq9w47hgpq39wu4hfqw4ufhwq4\
         3q8974fh q3489fh qopw4389fhpq9w4ghqpw94ghpqw94ufghpw9fhupq9w4ghpqw94ghpq\
         woeifjoweifjowijfow ejf owijf ejf qefasfoP OJP JP JE FPIJEPF IWJEPFI JWPEF W\
         WEIOFJ WEFJ WPEIGJH 0348HG39 84GHJF039 84JF0394JF0 384G0348HGOWEIRGJPRGOJPE\
         WEIFOJ WEOFIJ PQIEGHQPIGH024UHG034IUHJG0WIUEJF0EIWJGF0WEGH 0WEGH W0IEJF PWIEJFG PWEF\
         W0EFJ 0WEFJ -WIJF-024JG0F34IGJ03 4I JG03W4IJG02HG0IQJGW-EIGJWPIEJGWeuf";
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 1131\r\n\r\n", &String::from_utf8_lossy(body)],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    (CONTENT_LENGTH, "1131"),
                ],
                body: body.to_vec(),
            }))
    }

    #[test]
    fn header_multiple_colons() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\nhello: value: foo\r\n\r\n"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![
                    ("hello", "value: foo")
                ],
                body: vec![],
            }));
    }

    #[test]
    fn gibberish() {
        test_read_request(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn no_requests_read_after_bad_request() {
        test_read_request(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn lots_of_newlines() {
        test_read_request(
            vec!["\n\n\n\n\n", "\n\n\n", "\n\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn no_newlines() {
        test_read_request(
            vec!["wuirghuiwuhfwf", "iouwejf", "ioerjgiowjergiuhwelriugh"],
            Err(UnexpectedEOF.into()))
    }

    #[test]
    fn invalid_method() {
        test_read_request(
            vec!["yadadada / HTTP/1.1\r\n\r\n"],
            Err(UnrecognizedMethod("yadadada".to_string())))
    }

    #[test]
    fn wrong_http_version() {
        test_read_request(
            vec!["GET / HTTP/1.0\r\n\r\n"],
            Err(WrongHttpVersion.into()))
    }

    #[test]
    fn missing_uri_and_version() {
        test_read_request(
            vec!["GET\r\n\r\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn missing_http_version() {
        test_read_request(
            vec!["GET /\r\n\r\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn bad_crlf() {
        test_read_request(
            vec!["GET / HTTP/1.1\n\r\n"],
            Err(WrongHttpVersion.into()))
    }

    #[test]
    fn bad_header() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\nyadadada\r\n\r\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn header_with_newline() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\niwjfw\r\n\r\n"],
            Err(BadSyntax.into()))
    }

    #[test]
    fn missing_crlf_after_last_header() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\r\n"],
            Err(UnexpectedEOF.into()))
    }

    #[test]
    fn missing_crlfs() {
        test_read_request(
            vec!["GET / HTTP/1.1"],
            Err(UnexpectedEOF.into()))
    }

    #[test]
    fn body_no_content_length() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\n\r\nhello"],
            Ok(
                Request {
                    uri: String::from("/"),
                    method: Method::GET,
                    headers: HeaderMap::new(),
                    body: vec![],
                }))
    }

    #[test]
    fn body_too_short_content_length() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 3\r\n\r\nhello"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "3")],
                body: b"hel".to_vec(),
            }))
    }

    #[test]
    fn body_content_length_too_long() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello"],
            Err(Reading(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer")).into()))
    }

    #[test]
    fn body_content_length_too_long_request_after() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello",
                 "GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "10")],
                body: b"helloGET /".to_vec(),
            }))
    }

    #[test]
    fn negative_content_length() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: -5\r\n\r\nhello"],
            Err(InvalidHeaderValue.into()));
    }

    #[test]
    fn request_with_0_content_length() {
        test_read_request(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\n\r\nhello"],
            Ok(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "0")],
                body: vec![],
            }))
    }
}