//! The header `Kartenn-On-Behalf-Of` (spec 016, REQ-016-12; G10): a Dictionary Structured
//! Field (RFC 9651 §3.2) naming the requester a service account writes for: `sub`
//! (required), `iss` and `name` (optional), each a String or a Token (§3.3.3, 3.3.4).
//! Parameters are refused, unknown members ignored, the last of duplicates kept (§4.2.2).

/// Name of the header.
pub const ON_BEHALF_OF: &str = "kartenn-on-behalf-of";

pub use kartenn_service::authn::Requester;

/// Parses the value of the header.
///
/// # Errors
///
/// Why it is not a dictionary of the expected members.
pub fn parse(value: &str) -> Result<Requester, &'static str> {
    let mut input = value.trim_matches(' ');
    let (mut sub, mut iss, mut name) = (None, None, None);
    if input.is_empty() {
        return Err("Kartenn-On-Behalf-Of is empty");
    }
    loop {
        let (key, rest) = key(input)?;
        let rest = rest
            .strip_prefix('=')
            .ok_or("every member of Kartenn-On-Behalf-Of has a value")?;
        let (item, rest) = item(rest)?;
        if rest.starts_with(';') {
            return Err("Kartenn-On-Behalf-Of takes no parameters");
        }
        match key {
            "sub" => sub = Some(item),
            "iss" => iss = Some(item),
            "name" => name = Some(item),
            _ => {}
        }
        let rest = rest.trim_start_matches([' ', '\t']);
        if rest.is_empty() {
            break;
        }
        input = rest
            .strip_prefix(',')
            .ok_or("members of Kartenn-On-Behalf-Of are separated by commas")?
            .trim_start_matches([' ', '\t']);
        if input.is_empty() {
            return Err("Kartenn-On-Behalf-Of ends with a comma");
        }
    }
    let sub = sub
        .filter(|s| !s.is_empty())
        .ok_or("Kartenn-On-Behalf-Of needs a non-empty sub")?;
    Ok(Requester {
        sub,
        iss: iss.filter(|s| !s.is_empty()),
        name: name.filter(|s| !s.is_empty()),
    })
}

/// A key: `( lcalpha / "*" ) *( lcalpha / DIGIT / "_" / "-" / "." / "*" )` (§3.2).
fn key(input: &str) -> Result<(&str, &str), &'static str> {
    let first = input.chars().next().ok_or("a member has no key")?;
    if !(first.is_ascii_lowercase() || first == '*') {
        return Err("a key of Kartenn-On-Behalf-Of starts with a lowercase letter");
    }
    let end = input
        .find(|c: char| {
            !(c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '*'))
        })
        .unwrap_or(input.len());
    Ok(input.split_at(end))
}

/// A String (§3.3.3) or a Token (§3.3.4), and the rest.
fn item(input: &str) -> Result<(String, &str), &'static str> {
    if let Some(rest) = input.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = rest.char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some((_, e @ ('"' | '\\'))) => out.push(e),
                    _ => return Err("a string of Kartenn-On-Behalf-Of has a bad escape"),
                },
                '"' => return Ok((out, &rest[i + 1..])),
                ' '..='~' => out.push(c),
                _ => {
                    return Err("a string of Kartenn-On-Behalf-Of holds a non-printable character");
                }
            }
        }
        return Err("a string of Kartenn-On-Behalf-Of is not closed");
    }
    let first = input.chars().next().ok_or("a member has no value")?;
    if !(first.is_ascii_alphabetic() || first == '*') {
        return Err("a value of Kartenn-On-Behalf-Of is a string or a token");
    }
    // tchar / ":" / "/" (RFC 9110 §5.6.2).
    let end = input
        .find(|c: char| !(c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~:/".contains(c)))
        .unwrap_or(input.len());
    let (token, rest) = input.split_at(end);
    Ok((token.to_owned(), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dictionary_names_its_requester() {
        assert_eq!(
            parse(r#"sub="jdoe", iss="https://idp.example", name="J. Doe""#),
            Ok(Requester {
                sub: "jdoe".into(),
                iss: Some("https://idp.example".into()),
                name: Some("J. Doe".into()),
            })
        );
        // A token, an escape, an unknown member ignored, the last of duplicates kept.
        assert_eq!(
            parse(r#"sub=first,sub=a7, other=?1x, name="say \"hi\"""#).map(|r| (r.sub, r.name)),
            Err("a value of Kartenn-On-Behalf-Of is a string or a token")
        );
        assert_eq!(
            parse(r#"sub=first, sub=a7, other=x, name="say \"hi\"""#).map(|r| (r.sub, r.name)),
            Ok(("a7".into(), Some(r#"say "hi""#.into())))
        );
    }

    #[test]
    fn a_malformed_header_is_refused() {
        for bad in [
            "",
            "iss=x",
            "sub",
            r#"sub="""#,
            r#"sub="open"#,
            "sub=x;p=1",
            "sub=x,",
            "sub=x y",
            "Sub=x",
            r#"sub="a\nb""#,
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
