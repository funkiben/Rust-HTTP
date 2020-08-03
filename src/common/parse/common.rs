use std::io::{BufRead, Error, ErrorKind, Read};

use crate::common::header::{CONTENT_LENGTH, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};
use crate::common::parse::error::ParsingError;
use crate::common::parse::error_take::ErrorTake;
use crate::header_map;

/// The maximum size a line can be in an HTTP message.
/// A line is any data that is terminated by a CRLF, e.g. a header.
/// Without this limit, a connection may be kept open indefinitely if no new lines are sent.
const MAX_LINE_LENGTH: u64 = 512;

/// The maximum allowed length of all the headers in a request or response that can be parsed.
const MAX_HEADERS_LENGTH: u64 = 4096;

/// The maximum allowed length of a request or response body that can be parsed.
const MAX_BODY_LENGTH: u64 = 3 * 1024 * 1024; // 3 megabytes

/// Reads the first line, headers, and body of any HTTP request or response.
/// If a content length cannot be determined and read_if_no_content_length is true, then the
/// remainder of data in the reader will be put in the body (meaning this function will block until
/// the reader signals EOF). If it's false then no more data will be read after the headers, and the
/// body will be empty.
/// Returns a tuple of the first line of the request, the headers, and the body of the message.
pub fn read_message(reader: &mut impl BufRead, read_if_no_content_length: bool) -> Result<(String, HeaderMap, Vec<u8>), ParsingError> {
    let first_line = read_first_line(reader)?;
    let headers = read_headers(reader)?;
    let body = read_body(reader, &headers, read_if_no_content_length)?;

    Ok((first_line, headers, body))
}

/// Reads the first line of a message.
/// Maps UnexpectedEOF errors to EOF errors because EOF's are expected here.
/// An EOF found when reading the first line of a message means the connection has closed and nothing else will be transmitted.
fn read_first_line(reader: &mut impl BufRead) -> Result<String, ParsingError> {
    read_line(reader).map_err(|err|
        match err {
            ParsingError::Reading(err) if err.kind() == ErrorKind::UnexpectedEof => ParsingError::EOF,
            x => x
        }
    )
}

/// Reads a message body from the reader using the given headers.
fn read_body(reader: &mut impl BufRead, headers: &HeaderMap, read_if_no_content_length: bool) -> Result<Vec<u8>, ParsingError> {
    let mut reader = reader.error_take(MAX_BODY_LENGTH);

    if let Some(body_length) = get_content_length(headers) {
        read_body_with_length(&mut reader, body_length?)
    } else if is_chunked_transfer_encoding(headers) {
        read_chunked_body(&mut reader)
    } else if read_if_no_content_length {
        read_body_to_end(&mut reader)
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
fn read_chunked_body(reader: &mut impl BufRead) -> Result<Vec<u8>, ParsingError> {
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
/// If the line is empty and contains no CRLF, then a BadSyntax error is returned
fn read_line(reader: &mut impl BufRead) -> Result<String, ParsingError> {
    let mut line = String::new();
    reader.error_take(MAX_LINE_LENGTH).read_line(&mut line)?;

    if line.is_empty() {
        return Err(Error::from(ErrorKind::UnexpectedEof).into());
    }

    // pop the last two characters off and verify they're CRLF
    match (line.pop(), line.pop()) {
        (Some('\n'), Some('\r')) => Ok(line),
        _ => Err(ParsingError::BadSyntax)
    }
}

/// Reads and parses headers from the given reader.
fn read_headers(reader: &mut impl BufRead) -> Result<HeaderMap, ParsingError> {
    let mut reader = reader.error_take(MAX_HEADERS_LENGTH);

    let mut headers = header_map![];

    loop {
        let line = read_line(&mut reader)?;

        if line.is_empty() {
            return Ok(headers);
        }

        let (header, value) = parse_header(line)?;
        headers.add_header(header, value);
    }
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
    use crate::common::parse::common::read_message;
    use crate::common::parse::error::ParsingError::{BadSyntax, EOF, InvalidChunkSize, InvalidHeaderValue, Reading};
    use crate::common::parse::error::ParsingError;
    use crate::util::mock::{EndlessMockReader, MockReader};

    fn test_read_message(input: Vec<&str>, read_if_no_content_length: bool, expected_output: Result<(String, HeaderMap, Vec<u8>), ParsingError>) {
        let reader = MockReader::new(input);
        let mut reader = BufReader::new(reader);
        assert_eq!(format!("{:?}", read_message(&mut reader, read_if_no_content_length)), format!("{:?}", expected_output));
    }

    fn test_read_message_endless(data: Vec<&str>, endless_data: &str, read_if_no_content_length: bool, expected_output: Result<(String, HeaderMap, Vec<u8>), ParsingError>) {
        let reader = EndlessMockReader::new(data, endless_data);
        let mut reader = BufReader::new(reader);
        assert_eq!(format!("{:?}", read_message(&mut reader, read_if_no_content_length)), format!("{:?}", expected_output));
    }

    #[test]
    fn no_headers_or_body() {
        test_read_message(
            vec!["blah blah blah\r\n\r\n"],
            false,
            Ok(("blah blah blah".to_string(),
                Default::default(),
                vec![])),
        );
    }

    #[test]
    fn headers_and_body() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn headers_and_body_fragmented() {
        test_read_message(
            vec!["HTT", "P/1.", "1 200 OK", "\r", "\nconte", "nt-length", ":", " 5\r\n\r\nh", "el", "lo"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn only_one_message_returned() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello", "HTTP/1.1 200 OK\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(CONTENT_LENGTH, "5".to_string())]),
                "hello".as_bytes().to_vec())),
        );
    }

    #[test]
    fn big_body() {
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
    fn custom_header() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ncustom-header: custom header value\r\n\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(Header::Custom("custom-header".to_string()), "custom header value".to_string())]),
                vec![])),
        );
    }

    #[test]
    fn gibberish() {
        test_read_message(
            vec!["ergejrogi jerogij eworfgjwoefjwof9wef wfw"],
            false,
            Err(BadSyntax),
        );
    }

    #[test]
    fn gibberish_with_newline() {
        test_read_message(
            vec!["ergejrogi jerogij ewo\nrfgjwoefjwof9wef wfw"],
            false,
            Err(BadSyntax.into()),
        );
    }

    #[test]
    fn gibberish_with_crlf() {
        test_read_message(
            vec!["ergejrogi jerogij ewo\r\nrfgjwoefjwof9wef wfw\r\n\r\n"],
            false,
            Err(BadSyntax),
        );
    }

    #[test]
    fn gibberish_with_crlfs_at_end() {
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
    fn all_newlines() {
        test_read_message(
            vec!["\n\n\n\n\n\n\n\n\n\n\n"],
            false,
            Err(BadSyntax),
        );
    }

    #[test]
    fn all_crlfs() {
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
            Err(BadSyntax),
        );
    }

    #[test]
    fn only_one_crlf() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\n"],
            false,
            Err(Reading(Error::from(ErrorKind::UnexpectedEof))),
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
            Err(BadSyntax),
        );
    }

    #[test]
    fn one_crlf_nothing_else() {
        test_read_message(
            vec!["\r\n"],
            false,
            Err(Reading(Error::from(ErrorKind::UnexpectedEof))),
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
            Err(Reading(Error::from(ErrorKind::UnexpectedEof))),
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
            Err(BadSyntax),
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
            Err(BadSyntax),
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
            Err(Reading(Error::from(ErrorKind::UnexpectedEof))),
        );
    }

    #[test]
    fn chunked_body_only_chunk_size() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "2\r\n",
                 "he"],
            false,
            Err(Reading(Error::from(ErrorKind::UnexpectedEof))),
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

    #[test]
    fn chunked_body_huge_chunk() {
        let chunk = "eofjaiweughlwauehgliw uehfwaiuefhpqiwuefh lwieufh wle234532\
                 57rgoi jgoai\"\"\"woirjgowiejfiuf hawlieuf halweifu hawef awef \
                 weFIU HW iefu\t\r\n\r\nhweif uhweifuh qefq234523 812u9405834205 \
                 8245 1#@%^#$*&&^(*&)()&%^$%#^$]\r;g]ew r;g]ege\n\r\n\r\noweijf ow\
                 aiejf; aowiejf owf ifoa iwf aioerjf aoiwerjf laiuerwhgf lawiuefhj owfjdc\
                  wf                 awefoi jwaeoif jwei          WEAOFIJ AOEWI FJA EFJ  few\
                  wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                                  weiofj weoifj oweijfo qwiejfo quehfow uehfo qiwjfpo qihw fpqeighpqf efoiwej foq\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowi\r\nefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj ae\r\nlirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj ae\nlirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf\n oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowi\nefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
                 wefoi jawoiefj aowiefgj aelirugh aliowefj oaweijf oweijf owiejf oweifj weof\
         ";
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n",
                 "C2A\r\n",
                 chunk,
                 "\r\n",
                 "0\r\n",
                 "\r\n"],
            false,
            Ok(("HTTP/1.1 200 OK".to_string(),
                HeaderMap::from_pairs(vec![(TRANSFER_ENCODING, "chunked".to_string())]),
                chunk.as_bytes().to_vec())),
        );
    }

    #[test]
    fn huge_first_line() {
        test_read_message(
            vec!["HTTP/1.1 200 OKroig jseorgi jpseoriegj seorigj epoirgj epsigrj paweorgj aeo\
            6rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            4rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            3rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            2rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            1rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            4rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            8rgj seprogj aeorigj pserijg pseirjgp seijg aowijrg03w8u4 t0q83u40 qwifwagf awiorjgf aowi\
            9fj asodijv osdivj osidvja psijf pasidjf pas\r\n\
            content-length: 5\r\n\r\nhello"],
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );
    }

    #[test]
    fn huge_header() {
        test_read_message(
            vec!["HTTP/1.1 200 OK\r\n",
                 "big-header: iowjfo iawjeofiajw pefiawjpefoi hwjpeiUF HWPIU4FHPAIWUHGPAIWUHGP AIWUHGRP \
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            9Q43GHP 9Q3824U P9 658 23 YP 5698U24P985U2P198 4YU5P23985THPWERIUHG LIEAHVL DIFSJNV LAID\
            3JFHVL AIJFHVL AILIHiuh waiufh iefuhapergiu hapergiu hapeirug haeriug hsperg ",
                 "\r\n\r\n"],
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );
    }

    #[test]
    fn endless_line() {
        test_read_message_endless(
            vec![],
            "blah",
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        )
    }

    #[test]
    fn endless_headers() {
        test_read_message_endless(
            vec!["HTTP/1.1 200 OK\r\n"],
            "random: blah\r\n",
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );

        test_read_message_endless(
            vec!["HTTP/1.1 200 OK\r\n"],
            "random: blahh\r\n",
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );

        test_read_message_endless(
            vec!["HTTP/1.1 200 OK\r\n"],
            "random: blahhhh\r\n",
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );

        test_read_message_endless(
            vec!["HTTP/1.1 200 OK\r\n"],
            "a: a\r\n",
            false,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        );
    }

    #[test]
    fn endless_body() {
        test_read_message_endless(
            vec!["HTTP/1.1 200 OK\r\n\r\n"],
            "blah blah blah",
            true,
            Err(Reading(Error::new(ErrorKind::Other, "read limit reached"))),
        )
    }
}