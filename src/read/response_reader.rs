use std::io::BufRead;

use crate::common::{HTTP_VERSION, status};
use crate::common::header::HeaderMap;
use crate::common::response::Response;
use crate::common::status::Status;
use crate::header_map;
use crate::read::error::{ParsingError, ResponseParsingError};
use crate::read::message_reader::MessageReader;

pub struct ResponseReader<T> {
    inner: MessageReader<T, Response, ResponseParsingError>
}

impl<T: BufRead> ResponseReader<T> {
    pub fn new(inner: T) -> ResponseReader<T> {
        ResponseReader {
            inner: MessageReader::new(inner, true, get_default, parse_first_line, set_headers_and_body)
        }
    }

    pub fn read(&mut self) -> Result<Option<Response>, ResponseParsingError> {
        self.inner.read()
    }
}

fn parse_first_line(response: &mut Response, line: String) -> Result<(), ResponseParsingError> {
    let mut split = line.split(" ");

    let http_version = split.next().ok_or(ParsingError::BadSyntax)?;
    let status_code = split.next().ok_or(ParsingError::BadSyntax)?;

    if !http_version.eq(HTTP_VERSION) {
        return Err(ParsingError::WrongHttpVersion.into());
    }

    response.status = parse_status(status_code)?;

    Ok(())
}

fn set_headers_and_body(request: &mut Response, headers: HeaderMap, body: Vec<u8>) {
    request.headers = headers;
    request.body = body;
}

/// Parses the status code.
fn parse_status(code: &str) -> Result<Status, ResponseParsingError> {
    let code = code.parse().map_err(|_| ResponseParsingError::InvalidStatusCode)?;
    Status::from_code(code).ok_or(ResponseParsingError::InvalidStatusCode)
}

fn get_default() -> Response {
    Response {
        status: status::OK,
        headers: header_map![],
        body: vec![],
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps};
    use crate::common::response::Response;
    use crate::common::status;
    use crate::read::error::ParsingError::{BadSyntax, EOF, InvalidHeaderValue, Reading, WrongHttpVersion};
    use crate::read::error::ResponseParsingError;
    use crate::read::error::ResponseParsingError::InvalidStatusCode;
    use crate::read::response_reader::ResponseReader;
    use crate::util::mock::MockReader;

    fn test_with_eof(data: Vec<&str>, expected_result: Result<Response, ResponseParsingError>) {
        let reader = MockReader::from_strs(data);
        let reader = BufReader::new(reader);
        let actual_result = ResponseReader::new(reader).read();
        let actual_result = actual_result.map(|res| res.expect("Could not read full response"));
        match (expected_result, actual_result) {
            (Ok(exp), Ok(act)) => assert_eq!(exp, act),
            (exp, act) => assert_eq!(format!("{:?}", exp), format!("{:?}", act))
        }
    }

    #[test]
    fn no_headers_or_body() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Ok(Response {
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
            Err(InvalidStatusCode),
        );
    }

    #[test]
    fn negative_status_code() {
        test_with_eof(
            vec!["HTTP/1.1 -30 Not Found\r\n\r\n"],
            Err(InvalidStatusCode),
        );
    }

    #[test]
    fn gibberish_response() {
        test_with_eof(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn gibberish_with_newline() {
        test_with_eof(
            vec!["ergejrogi jerogij ewo\nrfgjwoefjwof9wef wfw"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn gibberish_with_crlf() {
        test_with_eof(
            vec!["ergejrogi jerogij ewo\r\nrfgjwoefjwof9wef wfw\r\n\r\n"],
            Err(WrongHttpVersion.into()),
        );
    }

    #[test]
    fn gibberish_with_crlfs_at_end() {
        test_with_eof(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw\r\n\r\n"],
            Err(WrongHttpVersion.into()),
        );
    }

    #[test]
    fn all_newlines() {
        test_with_eof(
            vec!["\n\n\n\n\n\n\n\n\n\n\n"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn all_crlfs() {
        test_with_eof(
            vec!["\r\n\r\n\r\n\r\n"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn wrong_http_version() {
        test_with_eof(
            vec!["HTTP/2.0 404 Not Found\r\n\r\n"],
            Err(WrongHttpVersion.into()),
        );
    }

    #[test]
    fn no_status_code() {
        test_with_eof(
            vec!["HTTP/1.1\r\n\r\n"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn missing_crlfs() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn only_one_crlf() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\n"],
            Err(Reading(Error::from(ErrorKind::UnexpectedEof)).into()),
        );
    }

    #[test]
    fn bad_header() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\nbad header\r\n\r\n"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn bad_content_length_value() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: five\r\n\r\nhello"],
            Err(InvalidHeaderValue.into()),
        );
    }

    #[test]
    fn no_data() {
        test_with_eof(
            vec![],
            Err(EOF.into()),
        );
    }

    #[test]
    fn one_character() {
        test_with_eof(
            vec!["a"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn one_crlf_nothing_else() {
        test_with_eof(
            vec!["\r\n"],
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn content_length_too_long() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello"],
            Err(Reading(Error::from(ErrorKind::UnexpectedEof)).into()),
        );
    }

    #[test]
    fn content_length_too_long_with_request_after() {
        test_with_eof(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n"],
            Ok(Response {
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
            Ok(Response {
                status: status::OK,
                headers: HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "3".to_string())]),
                body: "hel".as_bytes().to_vec(),
            }),
        );
    }
}