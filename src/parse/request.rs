use std::io::BufRead;

use crate::common::HTTP_VERSION;
use crate::common::method::Method;
use crate::common::request::Request;
use crate::parse::crlf_line::CrlfLineParser;
use crate::parse::error::ParsingError;
use crate::parse::message::MessageParser;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus::{Done, IoErr};

/// Parser for requests.
pub struct RequestParser(MessageParser<FirstLineParser, (Method, String)>);

impl RequestParser {
    /// Creates a new request parser.
    pub fn new() -> RequestParser {
        RequestParser(MessageParser::new(FirstLineParser::new(), false))
    }

    /// Returns true if this parser has read any data so far.
    pub fn has_data(&self) -> bool {
        self.0.first_line_parser().map(|p| { p.0.read_so_far() > 0 }).unwrap_or(true)
    }
}

impl Parse<Request> for RequestParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<Request, Self> {
        Ok(match self.0.parse(reader)? {
            Done(((method, uri), headers, body)) => Done(Request { method, uri, headers, body }),
            IoErr(parser, err) => IoErr(Self(parser), err)
        })
    }
}

/// The parser for the first line of a request.
struct FirstLineParser(CrlfLineParser);

impl FirstLineParser {
    /// Creates a new parser for a requests first line.
    fn new() -> FirstLineParser {
        FirstLineParser(CrlfLineParser::new())
    }
}

impl Parse<(Method, String)> for FirstLineParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<(Method, String), Self> {
        Ok(match self.0.parse(reader)? {
            Done(line) => Done(parse_first_line(line)?),
            IoErr(parser, err) => IoErr(Self(parser), err)
        })
    }
}

/// Parses the given string as the first line of a request. Verifies the HTTP version and returns the method and URI.
fn parse_first_line(line: String) -> Result<(Method, String), ParsingError> {
    let mut split = line.split(" ");

    let method_raw = split.next().ok_or(ParsingError::BadSyntax)?;
    let uri = split.next().ok_or(ParsingError::BadSyntax)?;
    let http_version = split.next().ok_or(ParsingError::BadSyntax)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ParsingError::WrongHttpVersion.into());
    }

    Ok((parse_method(method_raw)?, uri.to_string()))
}

/// Parses the given string into a method. If the method is not recognized, will return an error.
fn parse_method(raw: &str) -> Result<Method, ParsingError> {
    Method::try_from_str(raw).ok_or_else(|| ParsingError::UnrecognizedMethod)
}

#[cfg(test)]
mod tests {
    use std::io::{ErrorKind, BufReader};

    use crate::common::header::{CONNECTION, CONTENT_LENGTH, HeaderMap};
    use crate::common::method::Method;
    use crate::common::request::Request;
    use crate::header_map;
    use crate::parse::error::ParsingError::{BadSyntax, InvalidHeaderValue, UnrecognizedMethod, WrongHttpVersion};
    use crate::parse::request::RequestParser;
    use crate::parse::test_util;
    use crate::parse::test_util::TestParseResult;
    use crate::parse::test_util::TestParseResult::{ParseErr, Value};
    use crate::util::mock::MockReader;
    use crate::parse::parse::{Parse, ParseStatus};

    fn test_with_eof(data: Vec<&str>, expected: TestParseResult<Request>) {
        test_util::test_with_eof(RequestParser::new(), data, expected);
    }

    #[test]
    fn no_data() {
        test_with_eof(vec![], ErrorKind::UnexpectedEof.into());
    }

    #[test]
    fn no_header_or_body() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\n\r\n"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn no_header_or_body_fragmented() {
        test_with_eof(
            vec!["G", "ET / ", "HTTP/1", ".1\r\n", "\r", "\n"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn interesting_uri() {
        test_with_eof(
            vec!["GET /hello/world/ HTTP/1.1\r\n\r\n"],
            Value(Request {
                uri: String::from("/hello/world/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn weird_uri() {
        test_with_eof(
            vec!["GET !#%$#/-+=_$+[]{}\\%&$ HTTP/1.1\r\n\r\n"],
            Value(Request {
                uri: String::from("!#%$#/-+=_$+[]{}\\%&$"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn many_spaces_in_first_line() {
        test_with_eof(
            vec!["GET /hello/world/ HTTP/1.1 hello there blah blah\r\n\r\n"],
            Value(Request {
                uri: String::from("/hello/world/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn only_reads_one_request() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn headers() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\nconnection: close\r\nsomething: hello there goodbye\r\n\r\n"],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\ncontent-length: 0\r\nsomething: value 1\r\nsomething: value 2\r\n\r\n"],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncoNtEnt-lEngtH: 0\r\nCoNNECTION: close\r\nsoMetHing: hello there goodbye\r\n\r\n"],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\n: \r\n: \r\n\r\n"],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello"],
            Value(Request {
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
        test_with_eof(
            vec!["GE", "T / ", "HTT", "P/1.", "1\r", "\nconte", "nt-le", "n", "gth: ", "5\r\n\r", "\nhe", "ll", "o"],
            Value(Request {
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
        test_with_eof(
            vec![
                "GET /body1 HTTP/1.1\r\ncontent-length: 5\r\n\r\nhello",
                "GET /body2 HTTP/1.1\r\ncontent-length: 7\r\n\r\ngoodbye"
            ],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 1131\r\n\r\n", &String::from_utf8_lossy(body)],
            Value(Request {
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
        test_with_eof(
            vec!["GET / HTTP/1.1\r\nhello: value: foo\r\n\r\n"],
            Value(Request {
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
        test_with_eof(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn no_requests_read_after_bad_request() {
        test_with_eof(
            vec!["regw", "\nergrg\n", "ie\n\n\nwof\r\n\r\n", "POST / HTTP/1.1\r\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn lots_of_newlines() {
        test_with_eof(
            vec!["\n\n\n\n\n", "\n\n\n", "\n\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn no_newlines() {
        test_with_eof(
            vec!["wuirghuiwuhfwf", "iouwejf", "ioerjgiowjergiuhwelriugh"],
            ErrorKind::UnexpectedEof.into())
    }

    #[test]
    fn invalid_method() {
        test_with_eof(
            vec!["yadadada / HTTP/1.1\r\n\r\n"],
            ParseErr(UnrecognizedMethod))
    }

    #[test]
    fn wrong_http_version() {
        test_with_eof(
            vec!["GET / HTTP/1.0\r\n\r\n"],
            ParseErr(WrongHttpVersion))
    }

    #[test]
    fn missing_uri_and_version() {
        test_with_eof(
            vec!["GET\r\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn missing_http_version() {
        test_with_eof(
            vec!["GET /\r\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn bad_crlf() {
        test_with_eof(
            vec!["GET / HTTP/1.1\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn bad_header() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\nyadadada\r\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn header_with_newline() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\niwjfw\r\n\r\n"],
            ParseErr(BadSyntax))
    }

    #[test]
    fn missing_crlf_after_last_header() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\nhello: wgwf\r\n"],
            ErrorKind::UnexpectedEof.into())
    }

    #[test]
    fn missing_crlfs() {
        test_with_eof(
            vec!["GET / HTTP/1.1"],
            ErrorKind::UnexpectedEof.into())
    }

    #[test]
    fn body_no_content_length() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\n\r\nhello"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: HeaderMap::new(),
                body: vec![],
            }))
    }

    #[test]
    fn body_too_short_content_length() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 3\r\n\r\nhello"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "3")],
                body: b"hel".to_vec(),
            }))
    }

    #[test]
    fn body_content_length_too_long() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello"],
            ErrorKind::UnexpectedEof.into())
    }

    #[test]
    fn body_content_length_too_long_request_after() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello",
                 "GET / HTTP/1.1\r\ncontent-length: 10\r\n\r\nhello"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "10")],
                body: b"helloGET /".to_vec(),
            }))
    }

    #[test]
    fn negative_content_length() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: -5\r\n\r\nhello"],
            ParseErr(InvalidHeaderValue));
    }

    #[test]
    fn request_with_0_content_length() {
        test_with_eof(
            vec!["GET / HTTP/1.1\r\ncontent-length: 0\r\n\r\nhello"],
            Value(Request {
                uri: String::from("/"),
                method: Method::GET,
                headers: header_map![(CONTENT_LENGTH, "0")],
                body: vec![],
            }))
    }

    #[test]
    fn has_data_false() {
        let parser = RequestParser::new();
        assert!(!parser.has_data())
    }

    #[test]
    fn has_data_false_with_failed_read() {
        let parser = RequestParser::new();

        let mut reader = MockReader::from_strs(vec![]);
        reader.return_would_block_when_empty = true;

        let mut reader = BufReader::new(reader);

        match parser.parse(&mut reader) {
            Ok(ParseStatus::IoErr(parser, err)) if err.kind() == ErrorKind::WouldBlock => assert!(!parser.has_data()),
            _ => panic!("parse gave unexpected result")
        }
    }

    #[test]
    fn has_data_false_with_eof_read() {
        let parser = RequestParser::new();

        let reader = MockReader::from_strs(vec![""]);
        let mut reader = BufReader::new(reader);

        match parser.parse(&mut reader) {
            Ok(ParseStatus::IoErr(parser, err)) if err.kind() == ErrorKind::UnexpectedEof => assert!(!parser.has_data()),
            _ => panic!("parse gave unexpected result")
        }
    }

    #[test]
    fn has_data_true() {
        let parser = RequestParser::new();

        let mut reader = MockReader::from_strs(vec!["hello"]);
        reader.return_would_block_when_empty = true;

        let mut reader = BufReader::new(reader);

        match parser.parse(&mut reader) {
            Ok(ParseStatus::IoErr(parser, err)) if err.kind() == ErrorKind::WouldBlock => assert!(parser.has_data()),
            _ => panic!("parse gave unexpected result")
        }
    }

    #[test]
    fn has_data_true_more_than_first_line() {
        let parser = RequestParser::new();

        let mut reader = MockReader::from_strs(vec!["GET / HTTP/1.1\r\nhello: hi\r\n"]);
        reader.return_would_block_when_empty = true;

        let mut reader = BufReader::new(reader);

        match parser.parse(&mut reader) {
            Ok(ParseStatus::IoErr(parser, err)) if err.kind() == ErrorKind::WouldBlock => assert!(parser.has_data()),
            _ => panic!("parse gave unexpected result")
        }
    }
}
