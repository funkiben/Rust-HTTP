use std::collections::HashMap;
use std::io::{BufRead, BufReader, Error, Read};

use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};

/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
    /// Invalid syntax in the message.
    BadSyntax,
    /// Message has wrong HTTP version.
    WrongHttpVersion,
    /// Header has invalid value.
    InvalidHeaderValue,
    /// Unexpected EOF will be thrown when EOF is found in the middle of reading a request or response.
    UnexpectedEOF,
    /// EOF found before any request or response can be read.
    EOF,
    /// Size of chunk in chunked transfer encoding can not be parsed as a number.
    InvalidChunkSize,
    /// Error reading from the reader.
    Reading(Error),
}

impl From<Error> for ParsingError {
    fn from(err: Error) -> Self {
        ParsingError::Reading(err)
    }
}

/// Reads the first line, headers, and body of any HTTP request or response.
/// If a content length cannot be determined and read_if_no_content_length is true, then the
/// remainder of data in the reader will be put in the body (meaning this function will block until
/// the reader signals EOF). If it's false then no more data will be read after the headers, and the
/// body will be empty.
/// Returns a tuple of the first line of the request, the headers, and the body of the message.
pub fn read_message(reader: &mut BufReader<impl Read>, read_if_no_content_length: bool) -> Result<(String, HeaderMap, Vec<u8>), ParsingError> {
    let first_line = read_line(reader).map_err(|err|
        if let ParsingError::UnexpectedEOF = err { ParsingError::EOF } else { err }
    )?;

    let headers = parse_headers(read_lines_until_empty_line(reader)?)?;

    let body = read_body(reader, &headers, read_if_no_content_length)?;

    Ok((first_line, headers, body))
}

/// Reads a message body from the reader using the given headers.
fn read_body(reader: &mut BufReader<impl Read>, headers: &HeaderMap, read_if_no_content_length: bool) -> Result<Vec<u8>, ParsingError> {
    if let Some(body_length) = get_content_length(headers) {
        read_body_with_length(reader, body_length?)
    } else if is_chunked_transfer_encoding(headers) {
        read_chunked_body(reader)
    } else if read_if_no_content_length {
        read_body_to_end(reader)
    } else {
        Ok(Vec::new())
    }
}

/// Gets the value of a content-length header from the given header map. May return None if there's
/// no content-length header, or an error if the content-length value can not be parsed.
fn get_content_length(headers: &HeaderMap) -> Option<Result<usize, ParsingError>> {
    headers.get_first_header_value(&CONTENT_LENGTH)
        .map(|value| value.parse().map_err(|_| ParsingError::InvalidHeaderValue))
}

/// Checks if the header map has chunked transfer encoding header value.
fn is_chunked_transfer_encoding(headers: &HeaderMap) -> bool {
    headers.get_first_header_value(&TRANSFER_ENCODING).map(|v| v.eq("chunked")).unwrap_or(false)
}

/// Reads a message body from the reader with a defined length.
fn read_body_with_length(reader: &mut impl Read, body_length: usize) -> Result<Vec<u8>, ParsingError> {
    let mut buf = vec![0; body_length];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Reads a chunked body from the reader.
fn read_chunked_body(reader: &mut BufReader<impl Read>) -> Result<Vec<u8>, ParsingError> {
    let mut body = vec![];
    loop {
        let line = read_line(reader)?;
        let size = usize::from_str_radix(&line, 16).map_err(|_| ParsingError::InvalidChunkSize)?;

        let mut buf = vec![0; size];
        reader.read_exact(&mut buf)?;
        body.append(&mut buf);

        // get rid of crlf
        read_line(reader)?;

        if size == 0 {
            break;
        }
    }

    Ok(body)
}

/// Reads a message body from the reader. Reads until there's nothing left to read from.
fn read_body_to_end(reader: &mut impl Read) -> Result<Vec<u8>, ParsingError> {
    let mut buf = vec![];
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Reads a single line, assuming the line ends in a CRLF ("\r\n").
/// The CRLF is not included in the returned string.
fn read_line(reader: &mut BufReader<impl Read>) -> Result<String, ParsingError> {
    let mut line = String::new();
    reader.read_line(&mut line)?;

    if line.is_empty() {
        return Err(ParsingError::UnexpectedEOF);
    }

    // pop the \r\n off the end of the line
    line.pop();
    line.pop();

    Ok(line)
}

/// Reads lines from the buffered reader. The returned lines do not include a CRLF.
fn read_lines_until_empty_line(reader: &mut BufReader<impl Read>) -> Result<Vec<String>, ParsingError> {
    let mut lines = Vec::new();

    loop {
        let line = read_line(reader)?;

        if line.is_empty() {
            return Ok(lines);
        }

        lines.push(line);
    }
}

/// Tries to parse the given lines as headers.
/// Each line is parsed with the format "V: K" where V is the header name and K is the header value.
fn parse_headers(lines: Vec<String>) -> Result<HeaderMap, ParsingError> {
    let mut headers = HashMap::new();
    for line in lines {
        let (header, value) = parse_header(line)?;
        headers.add_header(header, value);
    }
    Ok(headers)
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
fn parse_header(raw: String) -> Result<(Header, String), ParsingError> {
    let mut split = raw.splitn(2, ": ");

    let header_raw = split.next().ok_or(ParsingError::BadSyntax)?;
    let value = split.next().ok_or(ParsingError::BadSyntax)?;

    Ok((Header::from(header_raw), String::from(value)))
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Error, ErrorKind};

    use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
    use crate::util::mock::MockReader;
    use crate::util::parse::{ParsingError, read_message};
    use crate::util::parse::ParsingError::{BadSyntax, EOF, InvalidChunkSize, InvalidHeaderValue, Reading, UnexpectedEOF};

    fn test_read_message(input: Vec<&str>, read_if_no_content_length: bool, expected_output: Result<(String, HeaderMap, Vec<u8>), ParsingError>) {
        let reader = MockReader::from(input);
        let mut reader = BufReader::new(reader);
        assert_eq!(format!("{:?}", read_message(&mut reader, read_if_no_content_length)), format!("{:?}", expected_output));
    }

    #[test]
    fn read_request_no_headers_or_body() {
        test_read_message(
            vec!["blah blah blah\r\n\r\n"],
            false,
            Ok(("blah blah blah".to_string(),
                Default::default(),
                vec![])),
        );
    }

    #[test]
    fn read_request_headers_and_body() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn read_request_headers_and_body_fragmented() {
        test_read_message(
            vec!["HTT", "P/1.", "1 200 OK", "\r", "\nconte", "nt-length", ":", " 5\r\n\r\nh", "el", "lo"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn read_only_one_request() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn read_request_long_body() {
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
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 1054\r\n\r\n", &String::from_utf8_lossy(body)],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "1054".to_string())]),
                body.to_vec())),
        );
    }

    #[test]
    fn read_if_no_content_length_true() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            true,
            Ok(("HTTP/1.1 200 OK".to_string(),
                Default::default(),
                "helloHTTP/1.1 200 OK\r\n\r\nHTTP/1.1 200 OK\r\n\r\n".as_bytes().to_vec())),
        );
    }

    #[test]
    fn read_if_no_content_length_false() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                Default::default(),
                vec![])),
        );
    }

    #[test]
    fn read_custom_header() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncustom-header: custom header value\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(Header::Custom("custom-header".to_string()), "custom header value".to_string())]),
                vec![])),
        );
    }

    #[test]
    fn read_gibberish_response() {
        test_read_message(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn read_gibberish_response_with_newline() {
        test_read_message(
            vec!["ergejrogi jerogij ewo\nrfgjwoefjwof9wef wfw"],
            false,
            Err(UnexpectedEOF.into()),
        );
    }

    #[test]
    fn read_gibberish_with_crlf() {
        test_read_message(
            vec!["ergejrogi jerogij ewo\r\nrfgjwoefjwof9wef wfw\r\n\r\n"],
            false,
            Err(BadSyntax),
        );
    }

    #[test]
    fn read_gibberish_with_crlfs_at_end() {
        test_read_message(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw\r\n\r\n"],
            false,
            Ok((
                "ergejrogi jerogij eworfgjwoefjwof9wef wfw".to_string(),
                Default::default(),
                vec![]
            )),
        );
    }

    #[test]
    fn read_all_newlines() {
        test_read_message(
            vec!["\n\n\n\n\n\n\n\n\n\n\n"],
            false,
            Ok(("".to_string(), Default::default(), vec![])),
        );
    }

    #[test]
    fn read_all_crlfs() {
        test_read_message(
            vec!["\r\n\r\n\r\n\r\n"],
            false,
            Ok(("".to_string(), Default::default(), vec![])),
        );
    }

    #[test]
    fn missing_crlfs() {
        test_read_message(
            vec!["HTTP/1.1 200 OK"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn only_one_crlf() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\n"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn bad_header() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\nbad header\r\n\r\n"],
            false,
            Err(BadSyntax),
        );
    }

    #[test]
    fn bad_content_length_value() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: five\r\n\r\nhello"],
            false,
            Err(InvalidHeaderValue),
        );
    }

    #[test]
    fn no_data() {
        test_read_message(
            vec![],
            false,
            Err(EOF),
        );
    }

    #[test]
    fn one_character() {
        test_read_message(
            vec!["a"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn one_crlf_nothing_else() {
        test_read_message(
            vec!["\r\n"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn content_length_too_long() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello"],
            false,
            Err(Reading(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer"))),
        );
    }

    #[test]
    fn content_length_too_long_with_request_after() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 7\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "7".to_string())]),
                "helloHT".as_bytes().to_vec())),
        );
    }

    #[test]
    fn content_length_too_short() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nhello"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "3".to_string())]),
                "hel".as_bytes().to_vec())),
        );
    }

    #[test]
    fn chunked_body() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "2\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(TRANSFER_ENCODING, "chunked".to_string())]),
                "hello world hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn chunked_body_no_termination() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "2\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn chunked_body_chunk_size_1_byte_too_large() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "3\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(TRANSFER_ENCODING, "chunked".to_string())]),
                "he\rllo world hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn chunked_body_chunk_size_2_bytes_too_large() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "4\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Err(InvalidChunkSize),
        );
    }

    #[test]
    fn chunked_body_chunk_size_many_bytes_too_large() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "13\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(TRANSFER_ENCODING, "chunked".to_string())]),
                "he\r\nc\r\nllo world hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn chunked_body_huge_chunk_size() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "100\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Err(Reading(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer"))),
        );
    }

    #[test]
    fn chunked_body_chunk_size_not_hex_digit() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "z\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Err(InvalidChunkSize),
        );
    }

    #[test]
    fn chunked_body_no_crlfs() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "zhelloiouf jwiufji ejif jef"],
            false,
            Err(InvalidChunkSize),
        );
    }


    #[test]
    fn chunked_body_no_content() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "9\r\n",
                 "\r\n"],
            false,
            Err(Reading(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer"))),
        );
    }

    #[test]
    fn chunked_body_no_trailing_crlf() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "2\r\n",
                 "he\r\n",
                 "c\r\n",
                 "llo world he\r\n",
                 "3\r\n",
                 "llo\r\n",
                 "0\r\n"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn chunked_body_only_chunk_size() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "2\r\n",
                 "he"],
            false,
            Err(UnexpectedEOF),
        );
    }

    #[test]
    fn empty_chunked_body() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(TRANSFER_ENCODING, "chunked".to_string())]),
                vec![])),
        );
    }
}