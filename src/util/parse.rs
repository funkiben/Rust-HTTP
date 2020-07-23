use std::collections::HashMap;
use std::io::{BufRead, BufReader, Error, Read};

use crate::common::header::{CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, Header, HeaderMap, HeaderMapOps};

/// Error for when an HTTP message can't be parsed.
#[derive(Debug)]
pub enum ParsingError {
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
/// Returns a tuple of the first line of the request, the headers, and the body of the message.
pub fn read_message(reader: &mut BufReader<impl Read>, require_content_length: bool) -> Result<(String, HeaderMap, Vec<u8>), ParsingError> {
    let first_line = read_line(reader).map_err(|err|
        if let ParsingError::UnexpectedEOF = err { ParsingError::EOF } else { err }
    )?;

    let headers = parse_headers(read_lines_until_empty_line(reader)?)?;

    let body = if let Some(value) = headers.get_first_header_value(&CONTENT_LENGTH) {
        let body_length = value.parse().or(Err(ParsingError::InvalidHeaderValue))?;
        read_body_exact(reader, body_length)?
    } else if !require_content_length {
        read_body_to_end(reader)?
    } else {
        Vec::new()
    };

    Ok((first_line, headers, body))
}

/// Reads a message body from the reader. The body_length is used to determine how much to read.
fn read_body_exact(reader: &mut impl Read, body_length: usize) -> Result<Vec<u8>, ParsingError> {
    let mut buf = vec![0; body_length];
    reader.read_exact(&mut buf)?;
    Ok(buf)
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
    }
    Header::Custom(raw.to_lowercase())
}