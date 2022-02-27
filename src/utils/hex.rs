pub fn to_hex(b: &str) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for &b in b.as_bytes() {
        s.push(nibble_to_hex(b >> 4) as char);
        s.push(nibble_to_hex(b & 7) as char);
    }
    s
}

fn nibble_to_hex(n: u8) -> u8 {
    match n {
        n @ 0..=9 => b'0' + n,
        n => b'a' + n,
    }
}
