use std::io::BufRead;

use crate::common::response::Response;
use crate::common::status::Status;
use crate::common::version;
use crate::parse::crlf_line::CrlfLineParser;
use crate::parse::error::ParsingError;
use crate::parse::message::MessageParser;
use crate::parse::parse::{Parse, ParseResult};
use crate::parse::parse::ParseStatus::{Done, IoErr};

/// Parser for responses.
pub struct ResponseParser(MessageParser<FirstLineParser, Status>);

impl ResponseParser {
    /// Returns a new response parser.
    pub fn new() -> ResponseParser {
        ResponseParser(MessageParser::new(FirstLineParser::new(), true))
    }
}

impl Parse<Response> for ResponseParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<Response, Self> {
        Ok(match self.0.parse(reader)? {
            Done((status, headers, body)) => Done(Response { status, headers, body }),
            IoErr(parser, err) => IoErr(Self(parser), err)
        })
    }
}

/// Parser for the first line of a response.
struct FirstLineParser(CrlfLineParser);

impl FirstLineParser {
    /// Creates a new parser for the first line of a response.
    fn new() -> FirstLineParser {
        FirstLineParser(CrlfLineParser::new())
    }
}

impl Parse<Status> for FirstLineParser {
    fn parse(self, reader: &mut impl BufRead) -> ParseResult<Status, Self> {
        Ok(match self.0.parse(reader)? {
            Done(line) => Done(parse_first_line(line)?),
            IoErr(parser, err) => IoErr(Self(parser), err)
        })
    }
}

/// Parses the given string as the first line of a response. Verifies the HTTP version and returns a status.
fn parse_first_line(line: String) -> Result<Status, ParsingError> {
    let mut split = line.split(" ");

    let http_version = split.next().ok_or(ParsingError::BadSyntax)?;
    let status_code = split.next().ok_or(ParsingError::BadSyntax)?;

    if !version::is_supported(http_version) {
        return Err(ParsingError::InvalidHttpVersion.into());
    }

    parse_status(status_code)
}

/// Parses the status code.
fn parse_status(code: &str) -> Result<Status, ParsingError> {
    let code = code.parse().map_err(|_| ParsingError::InvalidStatusCode)?;
    Status::from_code(code).ok_or(ParsingError::InvalidStatusCode)
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
    use crate::common::response::Response;
    use crate::common::status;
    use crate::parse::error::ParsingError::{BadSyntax, InvalidHeaderValue, InvalidHttpVersion, InvalidStatusCode};
    use crate::parse::response::ResponseParser;
    use crate::parse::test_util;
    use crate::parse::test_util::TestParseResult;
    use crate::parse::test_util::TestParseResult::{ParseErr, Value};

    fn test_with_eof(data: Vec<&str>, expected: TestParseResult<Response>) {
        test_util::test_with_eof(ResponseParser::new(), data, expected);
    }

    #[test]
    fn no_headers_or_body() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\n\r\n"],
            Value(Response {
                status: status::OK,
                headers: Default::default(),
                body: vec![],
            }),
        );
    }

    #[test]
    fn headers_and_body() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn headers_and_body_fragmented() {
        test_with_eof(
            vec!["HTT", "P/1.", "1 200 OK", "\r", "\nconte", "nt-length", ":", " 5\r\n\r\nh", "el", "lo"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn only_one_response_read() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                body: "hello".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn long_body() {
        let body = b"iuwrhgiuelrguihwleriughwleiruhglweiurhgliwerg fkwfowjeofjiwoefijwef \
        wergiuwehrgiuwehilrguwehlrgiuw fewfwferg wenrjg; weirng lwieurhg owieurhg oeiuwrhg oewirg er\
        gweuirghweiurhgleiwurhglwieurhglweiurhglewiurhto8w374yto8374yt9p18234u50982@#$%#$%^&%^*(^)&(\
        *)_)+__+*()*()&**^%&$##!~!@~``12]\n3'\']\\l[.'\"lk]/l;<:?<:}|?L:|?L|?|:?e       oivj        \
        \n\n\n\n\\\t\t\t\t\t\t\t\\\t\t\t\t                                                          \
        ioerjgfoiaejrogiaergq34t2345123`    oijrgoi wjergi jweorgi jweorgji                 eworigj \
        riogj ewoirgj oewirjg 934598ut6932458t\ruyo3485gh o4w589ghu w458                          9ghu\
        pw94358gh pw93458gh pw9345gh pw9438g\rhu pw3945hg pw43958gh pw495gh :::;wefwefwef wef we  e ;;\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        @#$%@#$^@#$%&#$@%^#$%@#$%@$^%$&$%^*^%&(^$%&*#%^$&@$%^#!#$!~```~~~```wefwef wef ee f efefe e{\
        P{P[p[p[][][][]{}{}][][%%%\n\n\n\n\n\n wefwfw e2123456768960798676reresdsxfbcgrtg eg erg   ";
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 1054\r\n\r\n", &String::from_utf8_lossy(body)],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "1054".to_string())]),
                body: body.to_vec(),
            }),
        );
    }

    #[test]
    fn no_content_length() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            Value(Response {
                status: status::OK,
                headers: Default::default(),
                body: "helloHTTP/1.1 200 OK\r\n\r\nHTTP/1.1 200 OK\r\n\r\n".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn custom_header() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncustom-header: custom header value\r\n\r\n"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(Header::Custom("custom-header".to_string()), "custom header value".to_string())]),
                body: vec![],
            }),
        );
    }

    #[test]
    fn not_found_404_response() {
        test_with_eof(
            vec!["HTTP/1.1 404 Not Found\r\n\r\n"],
            Value(Response {
                status: status::NOT_FOUND,
                headers: Default::default(),
                body: vec![],
            }),
        );
    }

    #[test]
    fn no_status_reason() {
        test_with_eof(
            vec!["HTTP/1.1 400\r\n\r\n"],
            Value(Response {
                status: status::BAD_REQUEST,
                headers: Default::default(),
                body: vec![],
            }),
        );
    }

    #[test]
    fn invalid_status_code() {
        test_with_eof(
            vec!["HTTP/1.1 300000 Not Found\r\n\r\n"],
            ParseErr(InvalidStatusCode),
        );
    }

    #[test]
    fn negative_status_code() {
        test_with_eof(
            vec!["HTTP/1.1 -30 Not Found\r\n\r\n"],
            ParseErr(InvalidStatusCode),
        );
    }

    #[test]
    fn gibberish_response() {
        test_with_eof(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw"],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn gibberish_with_newline() {
        test_with_eof(
            vec!["ergejrogi jerogij ewo\nrfgjwoefjwof9wef wfw"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn gibberish_with_crlf() {
        test_with_eof(
            vec!["ergejrogi jerogij ewo\r\nrfgjwoefjwof9wef wfw\r\n\r\n"],
            ParseErr(InvalidHttpVersion),
        );
    }

    #[test]
    fn gibberish_with_crlfs_at_end() {
        test_with_eof(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw\r\n\r\n"],
            ParseErr(InvalidHttpVersion),
        );
    }

    #[test]
    fn all_newlines() {
        test_with_eof(
            vec!["\n\n\n\n\n\n\n\n\n\n\n"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn all_crlfs() {
        test_with_eof(
            vec!["\r\n\r\n\r\n\r\n"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn wrong_http_version() {
        test_with_eof(
            vec!["HTTP/2.0 404 Not Found\r\n\r\n"],
            ParseErr(InvalidHttpVersion),
        );
    }

    #[test]
    fn no_status_code() {
        test_with_eof(
            vec!["HTTP/1.1\r\n\r\n"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn missing_crlfs() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK"],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn only_one_crlf() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\n"],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn bad_header() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\nbad header\r\n\r\n"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn bad_content_length_value() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: five\r\n\r\nhello"],
            ParseErr(InvalidHeaderValue),
        );
    }

    #[test]
    fn no_data() {
        test_with_eof(
            vec![],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn one_character() {
        test_with_eof(
            vec!["a"],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn one_crlf_nothing_else() {
        test_with_eof(
            vec!["\r\n"],
            ParseErr(BadSyntax),
        );
    }

    #[test]
    fn content_length_too_long() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello"],
            ErrorKind::UnexpectedEof.into(),
        );
    }

    #[test]
    fn content_length_too_long_with_request_after() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "7".to_string())]),
                body: "helloHT".as_bytes().to_vec(),
            }),
        );
    }

    #[test]
    fn content_length_too_short() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nhello"],
            Value(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "3".to_string())]),
                body: "hel".as_bytes().to_vec(),
            }),
        );
    }
}
