use crate::error::MailError;
use mail_parser::MessageParser;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ParsedEmailAddress {
    pub name: Option<String>,
    pub email: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InvalidEmailAddress {
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct DuplicateEmailAddress {
    pub raw: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ParseEmailAddressesResponse {
    pub addresses: Vec<ParsedEmailAddress>,
    pub invalid: Vec<InvalidEmailAddress>,
    pub duplicates: Vec<DuplicateEmailAddress>,
}

pub fn parse_email_addresses(input: String) -> Result<ParseEmailAddressesResponse, MailError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(ParseEmailAddressesResponse {
            addresses: Vec::new(),
            invalid: Vec::new(),
            duplicates: Vec::new(),
        });
    }

    let mut seen = HashSet::new();
    let mut response = ParseEmailAddressesResponse {
        addresses: Vec::new(),
        invalid: Vec::new(),
        duplicates: Vec::new(),
    };

    for segment in split_address_segments(trimmed) {
        parse_segment(&segment, &mut seen, &mut response);
    }

    Ok(response)
}

fn parse_segment(
    segment: &str,
    seen: &mut HashSet<String>,
    response: &mut ParseEmailAddressesResponse,
) {
    let segment = segment.trim();
    if segment.is_empty() {
        return;
    }

    let parsed = parse_with_mail_parser(segment);
    if parsed.is_empty() {
        response.invalid.push(InvalidEmailAddress {
            raw: segment.to_string(),
            reason: "无法解析邮件地址".to_string(),
        });
        return;
    }

    for address in parsed {
        let dedupe_key = address.email.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            response.addresses.push(address);
        } else {
            response.duplicates.push(DuplicateEmailAddress {
                raw: address.raw,
                email: address.email,
            });
        }
    }
}

fn parse_with_mail_parser(segment: &str) -> Vec<ParsedEmailAddress> {
    let raw = format!("To: {segment}\r\n\r\n");
    let Some(message) = MessageParser::default().parse(raw.as_bytes()) else {
        return Vec::new();
    };
    let Some(addresses) = message.to() else {
        return Vec::new();
    };

    addresses
        .iter()
        .filter_map(|addr| {
            let email = addr.address()?.to_string();
            Some(ParsedEmailAddress {
                name: addr.name().map(ToString::to_string),
                raw: format_raw_address(addr.name(), &email),
                email,
            })
        })
        .collect()
}

fn format_raw_address(name: Option<&str>, email: &str) -> String {
    match name {
        Some(name) if !name.trim().is_empty() => format!("{name} <{email}>"),
        _ => email.to_string(),
    }
}

fn split_address_segments(input: &str) -> Vec<String> {
    let splitter = AddressSplitter::default();
    splitter.split(input)
}

#[derive(Default)]
struct AddressSplitter {
    segments: Vec<String>,
    current: String,
    in_quote: bool,
    escaped: bool,
    angle_depth: usize,
    comment_depth: usize,
    group_depth: usize,
}

impl AddressSplitter {
    fn split(mut self, input: &str) -> Vec<String> {
        for ch in input.chars() {
            self.push_char(ch);
        }
        self.flush_current();
        self.segments
    }

    fn push_char(&mut self, ch: char) {
        if self.escaped {
            self.current.push(ch);
            self.escaped = false;
            return;
        }

        if self.in_quote {
            self.current.push(ch);
            if ch == '\\' {
                self.escaped = true;
            } else if ch == '"' {
                self.in_quote = false;
            }
            return;
        }

        match ch {
            '"' => {
                self.in_quote = true;
                self.current.push(ch);
            }
            '<' => {
                self.angle_depth += 1;
                self.current.push(ch);
            }
            '>' => {
                self.angle_depth = self.angle_depth.saturating_sub(1);
                self.current.push(ch);
            }
            '(' => {
                self.comment_depth += 1;
                self.current.push(ch);
            }
            ')' => {
                self.comment_depth = self.comment_depth.saturating_sub(1);
                self.current.push(ch);
            }
            ':' if self.is_top_level() => {
                self.current.clear();
                self.group_depth += 1;
            }
            ';' if self.group_depth > 0 && self.is_group_member_level() => {
                self.flush_current();
                self.group_depth -= 1;
            }
            ',' if self.is_separator_level() => {
                self.flush_current();
            }
            _ => self.current.push(ch),
        }
    }

    fn is_top_level(&self) -> bool {
        self.angle_depth == 0 && self.comment_depth == 0 && self.group_depth == 0
    }

    fn is_group_member_level(&self) -> bool {
        self.angle_depth == 0 && self.comment_depth == 0
    }

    fn is_separator_level(&self) -> bool {
        self.angle_depth == 0 && self.comment_depth == 0
    }

    fn flush_current(&mut self) {
        let segment = self.current.trim();
        if !segment.is_empty() {
            self.segments.push(segment.to_string());
        }
        self.current.clear();
    }
}
