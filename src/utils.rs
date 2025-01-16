use std::str::Chars;

pub fn string_match(haystack: &[char], start: usize, needle: Chars, _regex: bool) -> Option<usize> {
    let mut last = None;
    for (i, nc) in needle.enumerate() {
        if haystack.get(start + i).is_none_or(|hc| *hc != nc) {
            return None;
        }
        last = last.or(Some(start));
        last = Some(last.unwrap() + 1);
    }
    Some(last.unwrap() - 1)
}

pub struct Log<'a>(pub &'a mlua::Lua);

impl<'a> Log<'a> {
    fn method(&self, fn_name: &str, message: &str) -> mlua::Result<()> {
        let log = self.0.globals().get::<mlua::Table>("LOG")?;
        let warn = log.get::<mlua::Function>(fn_name)?;
        warn.call::<()>((log, message))
    }

    pub fn error(&self, message: &str) -> mlua::Result<()> {
        self.method("error", message)
    }
    pub fn info(&self, message: &str) -> mlua::Result<()> {
        self.method("info", message)
    }
    pub fn warn(&self, message: &str) -> mlua::Result<()> {
        self.method("warn", message)
    }
    pub fn debug(&self, message: &str) -> mlua::Result<()> {
        self.method("debug", message)
    }
}

pub fn lua_error(lua: &mlua::Lua, message: &str) -> mlua::Result<()> {
    Log(lua).error(message)
}

pub fn lua_info(lua: &mlua::Lua, message: &str) -> mlua::Result<()> {
    Log(lua).info(message)
}

pub fn lua_warn(lua: &mlua::Lua, message: &str) -> mlua::Result<()> {
    Log(lua).warn(message)
}

pub fn lua_debug(lua: &mlua::Lua, message: &str) -> mlua::Result<()> {
    Log(lua).debug(message)
}

#[cfg(test)]
mod tests {
    use crate::utils::string_match;

    #[test]
    fn test_string_match() {
        let haystack = ". , . , . , . ,".chars().collect::<Vec<_>>();
        let key1 = ".";
        let key2 = " ,";

        let p1 = string_match(&haystack, 0, key1.chars(), false);
        assert_eq!(p1, Some(0));

        let p2 = string_match(&haystack, p1.unwrap() + 1, key2.chars(), false);
        assert_eq!(p2, Some(2));

        let p3 = string_match(&haystack, p2.unwrap() + 1, key2.chars(), false);
        assert_eq!(p3, None);

        let p4 = string_match(&haystack, p2.unwrap() + 1, key1.chars(), false);
        assert_eq!(p4, None);

        let p5 = string_match(&haystack, p2.unwrap() + 2, key1.chars(), false);
        assert_eq!(p5, Some(4));

        let p6 = string_match(&haystack, 13, key2.chars(), false);
        assert_eq!(p6, Some(haystack.len() - 1));

        let p7 = string_match(&haystack, 14, key2.chars(), false);
        assert_eq!(p7, None);
    }
}
