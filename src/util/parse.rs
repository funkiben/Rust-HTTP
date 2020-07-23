use std::collections::HashMap;
use std::io::{BufRead, BufReader, Error, Read};

use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps, TRANSFER_ENCODING};

/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
    // TODO compound some of these into just a "BadSyntax" error?
    /// Problem parsing a header.
    BadHeader,
    /// Message has wrong HTTP version.
    WrongHttpVersion,
    /// Missing HTTP version from first line of message.
    MissingHttpVersion,
    /// Header has invalid value.
    InvalidHeaderValue,
    /// Unexpected EOF will be thrown when EOF is found in the middle of reading a request or response.
    UnexpectedEOF,
    /// EOF found before any request or response can be read.
    EOF,
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
        // TODO change error
        let size = usize::from_str_radix(&line, 16).map_err(|_| ParsingError::InvalidHeaderValue)?;

        if size == 0 {
            break;
        }

        let mut buf = vec![0; size];
        reader.read_exact(&mut buf)?;
        body.append(&mut buf);

        // get rid of crlf
        read_line(reader)?;
    }

    // get rid of trailing crlf
    read_line(reader)?;

    Ok(body)
}

/// Reads a message body from the reader. Reads until there's nothing left to read from.
pub fn read_body_to_end(reader: &mut impl Read) -> Result<Vec<u8>, ParsingError> {
    let mut buf = vec![];
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Reads a single line, assuming the line ends in a CRLF ("\r\n").
/// The CRLF is not included in the returned string.
pub fn read_line(reader: &mut BufReader<impl Read>) -> Result<String, ParsingError> {
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
pub fn read_lines_until_empty_line(reader: &mut BufReader<impl Read>) -> Result<Vec<String>, ParsingError> {
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
pub fn parse_headers(lines: Vec<String>) -> Result<HeaderMap, ParsingError> {
    let mut headers = HashMap::new();
    for line in lines {
        let (header, value) = parse_header(line)?;
        headers.add_header(header, value);
    }
    Ok(headers)
}

/// Parses the given line as a header. Splits the line at the first ": " pattern.
pub fn parse_header(raw: String) -> Result<(Header, String), ParsingError> {
    let mut split = raw.splitn(2, ": ");

    let header_raw = split.next().ok_or(ParsingError::BadHeader)?;
    let value = split.next().ok_or(ParsingError::BadHeader)?;

    Ok((parse_header_name(header_raw), String::from(value)))
}

/// Parses the given header name. Will try to use a predefined header constant if possible to save memory.
/// Otherwise, will return a Custom header.
pub fn parse_header_name(raw: &str) -> Header {
    // TODO
    if "connection".eq_ignore_ascii_case(raw) {
        return CONNECTION;
    } else if "content-length".eq_ignore_ascii_case(raw) {
        return CONTENT_LENGTH;
    } else if "content-type".eq_ignore_ascii_case(raw) {
        return CONTENT_TYPE;
    } else if "transfer-encoding".eq_ignore_ascii_case(raw) {
        return TRANSFER_ENCODING;
    }
    Header::Custom(raw.to_lowercase())
}