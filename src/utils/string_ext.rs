use isnt::std_1::primitive::IsntStrExt;

pub trait StringExt {
    fn to_string_if_not_empty(&self) -> Option<String>;
}

impl StringExt for str {
    fn to_string_if_not_empty(&self) -> Option<String> {
        self.is_not_empty().then(|| self.to_string())
    }
}
