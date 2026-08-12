use crate::open;
use crate::str_table::intern;
use anyhow::Result;
use std::io::Write;

pub fn main() -> Result<()> {
    let mut f = open("time_lt.rs")?;
    writeln!(f, "use crate::utils::str_table::StrAccess;")?;
    writeln!(f)?;
    writeln!(f, "static LT: [StrAccess; 100] = [")?;
    for i in 0..=99 {
        let s = format!("{:02}", i);
        writeln!(f, "    {},", intern(&s))?;
    }
    writeln!(f, "];")?;
    Ok(())
}
