use isnt::std_1::primitive::IsntStrExt;

pub trait StringExt {
    fn to_string_if_not_empty(&self) -> Option<String>;
}

impl StringExt for str {
    fn to_string_if_not_empty(&self) -> Option<String> {
        self.is_not_empty().then(|| self.to_string())
    }
}

#[expect(unused)]
pub trait StringVecExt {
    fn into_empty_string(self) -> String;
}

impl StringVecExt for Vec<u8> {
    fn into_empty_string(mut self) -> String {
        self.clear();
        unsafe { String::from_utf8_unchecked(self) }
    }
}
