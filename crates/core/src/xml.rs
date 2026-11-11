// SPDX-License-Identifier: GPL-3.0-or-later
//! A tiny XML reader (elements, attributes, comments, declarations, the five predefined
//! entities). Enough for VocalTractLab `.speaker` files; no namespaces, DTDs or CDATA.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn attr_f64(&self, key: &str) -> Result<f64, String> {
        let v = self.attr(key).ok_or_else(|| format!("<{}> has no attribute '{}'", self.name, key))?;
        v.trim().parse::<f64>().map_err(|_| format!("<{}> attribute '{}' is not a number: '{}'", self.name, key, v))
    }

    pub fn child(&self, name: &str) -> Result<&Node, String> {
        self.children.iter().find(|c| c.name == name).ok_or_else(|| format!("<{}> has no child <{}>", self.name, name))
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
}

pub fn parse(text: &str) -> Result<Node, String> {
    let b = text.as_bytes();
    let mut pos = 0;
    skip_misc(b, &mut pos)?;
    let root = parse_element(text, b, &mut pos)?;
    skip_misc(b, &mut pos)?;
    if pos < b.len() {
        return Err(format!("unexpected content after the root element at byte {pos}"));
    }
    Ok(root)
}

fn skip_ws(b: &[u8], pos: &mut usize) {
    while *pos < b.len() && b[*pos].is_ascii_whitespace() { *pos += 1; }
}

/// Skips whitespace, comments and `<? ... ?>` declarations.
fn skip_misc(b: &[u8], pos: &mut usize) -> Result<(), String> {
    loop {
        skip_ws(b, pos);
        if b[*pos..].starts_with(b"<!--") {
            *pos = find(b, *pos + 4, b"-->").ok_or("unterminated comment")? + 3;
        } else if b[*pos..].starts_with(b"<?") {
            *pos = find(b, *pos + 2, b"?>").ok_or("unterminated declaration")? + 2;
        } else {
            return Ok(());
        }
    }
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    (from..b.len().saturating_sub(pat.len() - 1)).find(|&i| &b[i..i + pat.len()] == pat)
}

fn name_end(b: &[u8], mut p: usize) -> usize {
    while p < b.len() && (b[p].is_ascii_alphanumeric() || matches!(b[p], b'_' | b'-' | b':' | b'.')) { p += 1; }
    p
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn parse_element(text: &str, b: &[u8], pos: &mut usize) -> Result<Node, String> {
    if *pos >= b.len() || b[*pos] != b'<' { return Err(format!("expected '<' at byte {}", *pos)); }
    *pos += 1;
    let ne = name_end(b, *pos);
    if ne == *pos { return Err(format!("empty element name at byte {}", *pos)); }
    let mut node = Node { name: text[*pos..ne].to_string(), ..Node::default() };
    *pos = ne;
    loop {
        skip_ws(b, pos);
        if *pos >= b.len() { return Err(format!("unterminated tag <{}>", node.name)); }
        match b[*pos] {
            b'/' => {
                if b.get(*pos + 1) != Some(&b'>') { return Err(format!("malformed end of <{}>", node.name)); }
                *pos += 2;
                return Ok(node);
            }
            b'>' => { *pos += 1; break; }
            _ => {
                let ke = name_end(b, *pos);
                if ke == *pos { return Err(format!("bad attribute in <{}> at byte {}", node.name, *pos)); }
                let key = text[*pos..ke].to_string();
                *pos = ke;
                skip_ws(b, pos);
                if b.get(*pos) != Some(&b'=') { return Err(format!("attribute '{key}' has no value")); }
                *pos += 1;
                skip_ws(b, pos);
                let q = *b.get(*pos).ok_or("unexpected end")?;
                if q != b'"' && q != b'\'' { return Err(format!("attribute '{key}' is not quoted")); }
                let end = find(b, *pos + 1, &[q]).ok_or("unterminated attribute value")?;
                node.attrs.push((key, unescape(&text[*pos + 1..end])));
                *pos = end + 1;
            }
        }
    }
    // children until the matching end tag; text content is ignored
    loop {
        while *pos < b.len() && b[*pos] != b'<' { *pos += 1; }
        if *pos >= b.len() { return Err(format!("missing end tag for <{}>", node.name)); }
        if b[*pos..].starts_with(b"<!--") {
            *pos = find(b, *pos + 4, b"-->").ok_or("unterminated comment")? + 3;
        } else if b[*pos..].starts_with(b"</") {
            let ne = name_end(b, *pos + 2);
            if &text[*pos + 2..ne] != node.name { return Err(format!("mismatched end tag </{}> for <{}>", &text[*pos + 2..ne], node.name)); }
            *pos = find(b, ne, b">").ok_or("unterminated end tag")? + 1;
            return Ok(node);
        } else {
            node.children.push(parse_element(text, b, pos)?);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_elements_attributes_and_comments() {
        let n = parse("<?xml version=\"1.0\"?>\n<a x=\"1\" y='two &amp; three'><!-- c --><b/><b z=\"3.5\"></b><c>text</c></a>\n").unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attr("y"), Some("two & three"));
        assert_eq!(n.children_named("b").count(), 2);
        assert_eq!(n.children_named("b").nth(1).unwrap().attr_f64("z").unwrap(), 3.5);
        assert!(n.child("c").is_ok() && n.child("d").is_err());
    }

    #[test]
    fn reports_errors() {
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a x=1/>").is_err());
        assert!(parse("<a>").is_err());
        assert!(parse("<a/><b/>").is_err());
    }
}
