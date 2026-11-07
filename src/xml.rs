//! A small XML reader and escaping helpers, enough for spreadsheet file parts.

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub kids: Vec<Node>,
    pub text: String,
}

impl Node {
    /// Local part of the element name, without its namespace prefix.
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name || k.rsplit(':').next() == Some(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn attr_f(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|x| x.parse().ok())
    }
    pub fn attr_u(&self, name: &str) -> Option<u32> {
        self.attr(name).and_then(|x| x.parse().ok())
    }
    pub fn attr_on(&self, name: &str) -> bool {
        matches!(self.attr(name), Some("1") | Some("true"))
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.local() == name)
    }
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.kids.iter().filter(move |k| k.local() == name)
    }
    /// First descendant with this local name.
    pub fn find(&self, name: &str) -> Option<&Node> {
        for k in &self.kids {
            if k.local() == name {
                return Some(k);
            }
            if let Some(x) = k.find(name) {
                return Some(x);
            }
        }
        None
    }
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for k in &self.kids {
            if k.local() == name {
                out.push(k);
            }
            k.find_all(name, out);
        }
    }
    /// All text under this node.
    pub fn all_text(&self) -> String {
        let mut s = self.text.clone();
        for k in &self.kids {
            s.push_str(&k.all_text());
        }
        s
    }
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(j) = tail.find(';') {
            let ent = &tail[1..j];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => {
                    if let Some(h) = ent.strip_prefix("#x") {
                        u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
                    } else if let Some(d) = ent.strip_prefix('#') {
                        d.parse::<u32>().ok().and_then(char::from_u32)
                    } else {
                        None
                    }
                }
            };
            match rep {
                Some(c) => out.push(c),
                None => out.push_str(&tail[..=j]),
            }
            rest = &tail[j + 1..];
        } else {
            out.push_str(tail);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            c if (c as u32) < 32 && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// Parses a document into a tree and returns the root element.
pub fn parse(src: &str) -> Result<Node, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut stack: Vec<Node> = vec![Node::default()];
    while i < b.len() {
        if b[i] != b'<' {
            let st = i;
            while i < b.len() && b[i] != b'<' {
                i += 1;
            }
            let t = &src[st..i];
            if let Some(top) = stack.last_mut() {
                if !t.trim().is_empty() || top.name.ends_with(":t") || top.name == "t" || top.name.ends_with(":p") || top.name.ends_with(":s") {
                    top.text.push_str(&unescape(t));
                }
            }
            continue;
        }
        if src[i..].starts_with("<?") {
            i = src[i..].find("?>").map_or(b.len(), |x| i + x + 2);
            continue;
        }
        if src[i..].starts_with("<!--") {
            i = src[i..].find("-->").map_or(b.len(), |x| i + x + 3);
            continue;
        }
        if src[i..].starts_with("<![CDATA[") {
            let e = src[i..].find("]]>").map_or(b.len(), |x| i + x);
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&src[i + 9..e]);
            }
            i = e + 3;
            continue;
        }
        if src[i..].starts_with("<!") {
            i = src[i..].find('>').map_or(b.len(), |x| i + x + 1);
            continue;
        }
        if src[i..].starts_with("</") {
            let e = src[i..].find('>').ok_or("Unclosed tag")? + i;
            i = e + 1;
            let n = stack.pop().ok_or("Bad nesting")?;
            stack.last_mut().ok_or("Bad nesting")?.kids.push(n);
            continue;
        }
        // Start tag.
        let mut j = i + 1;
        let mut quote = 0u8;
        while j < b.len() {
            let c = b[j];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                quote = c;
            } else if c == b'>' {
                break;
            }
            j += 1;
        }
        if j >= b.len() {
            return Err("Unclosed tag".into());
        }
        let self_close = b[j - 1] == b'/';
        let inner = &src[i + 1..if self_close { j - 1 } else { j }];
        let mut node = Node::default();
        let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
        node.name = inner[..name_end].to_string();
        let mut rest = inner[name_end..].trim_start();
        while !rest.is_empty() {
            let Some(eq) = rest.find('=') else { break };
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let q = after.chars().next().unwrap_or('"');
            let val_end = after[1..].find(q).map_or(after.len() - 1, |x| x);
            node.attrs.push((key, unescape(&after[1..1 + val_end])));
            rest = after.get(val_end + 2..).unwrap_or("").trim_start();
        }
        i = j + 1;
        if self_close {
            stack.last_mut().ok_or("Bad nesting")?.kids.push(node);
        } else {
            stack.push(node);
        }
    }
    let mut root = stack.pop().ok_or("Empty document")?;
    root.kids.pop().ok_or_else(|| "No root element".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tree() {
        let n = parse("<?xml version=\"1.0\"?><a x=\"1 &amp; 2\"><b>hi</b><c:d e='f'/></a>").unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attr("x"), Some("1 & 2"));
        assert_eq!(n.child("b").unwrap().text, "hi");
        assert_eq!(n.child("d").unwrap().attr("e"), Some("f"));
    }
}
