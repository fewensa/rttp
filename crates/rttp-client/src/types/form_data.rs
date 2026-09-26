use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum FormDataType {
  TEXT,
  FILE,
  BINARY,
}

pub trait ToFormData {
  fn to_formdatas(&self) -> Vec<FormData>;
}

#[derive(Clone, Debug)]
pub struct FormData {
  name: String,
  text: Option<String>,
  file: Option<PathBuf>,
  filename: Option<String>,
  binary: Vec<u8>,
  type_: FormDataType,
  array: bool,
}

#[allow(dead_code)]
impl FormData {
  pub fn with_text<S: AsRef<str>, T: AsRef<str>>(name: S, text: T) -> Self {
    Self {
      name: name.as_ref().into(),
      text: Some(text.as_ref().into()),
      file: None,
      filename: None,
      binary: vec![],
      type_: FormDataType::TEXT,
      array: false,
    }
  }

  pub fn with_file<S: AsRef<str>, P: AsRef<Path>>(name: S, file: P) -> Self {
    let file = file.as_ref();
    let filename = file
      .file_name()
      .map_or("".to_string(), |v| v.to_string_lossy().to_string());
    Self::with_file_and_name(name, file, filename)
  }

  pub fn with_file_and_name<S: AsRef<str>, N: AsRef<str>, P: AsRef<Path>>(
    name: S,
    file: P,
    filename: N,
  ) -> Self {
    let filename = filename.as_ref();
    let filename = if filename.is_empty() {
      None
    } else {
      Some(filename.to_string())
    };
    Self {
      name: name.as_ref().into(),
      text: None,
      file: Some(file.as_ref().to_path_buf()),
      filename,
      binary: vec![],
      type_: FormDataType::FILE,
      array: false,
    }
  }

  pub fn with_binary<S: AsRef<str>>(name: S, binary: Vec<u8>) -> Self {
    Self {
      name: name.as_ref().into(),
      text: None,
      file: None,
      filename: None,
      binary,
      type_: FormDataType::BINARY,
      array: false,
    }
  }

  pub fn name(&self) -> &String {
    &self.name
  }
  pub fn text(&self) -> &Option<String> {
    &self.text
  }
  pub fn file(&self) -> &Option<PathBuf> {
    &self.file
  }
  pub fn filename(&self) -> &Option<String> {
    &self.filename
  }
  pub fn binary(&self) -> &Vec<u8> {
    &self.binary
  }
  pub fn type_(&self) -> &FormDataType {
    &self.type_
  }
  pub fn array(&self) -> bool {
    self.array
  }

  pub fn is_text(&self) -> bool {
    self.type_ == FormDataType::TEXT
  }
  pub fn is_file(&self) -> bool {
    self.type_ == FormDataType::FILE
  }
  pub fn is_binary(&self) -> bool {
    self.type_ == FormDataType::BINARY
  }

  pub(crate) fn name_mut(&mut self) -> &mut String {
    &mut self.name
  }
  pub(crate) fn text_mut(&mut self) -> &mut Option<String> {
    &mut self.text
  }
  pub(crate) fn file_mut(&mut self) -> &mut Option<PathBuf> {
    &mut self.file
  }
  pub(crate) fn filename_mut(&mut self) -> &mut Option<String> {
    &mut self.filename
  }
  pub(crate) fn binary_mut(&mut self) -> &mut Vec<u8> {
    &mut self.binary
  }
  pub(crate) fn type_mut(&mut self) -> &mut FormDataType {
    &mut self.type_
  }
  pub(crate) fn array_mut(&mut self) -> &mut bool {
    &mut self.array
  }
}

impl ToFormData for FormData {
  fn to_formdatas(&self) -> Vec<FormData> {
    vec![self.clone()]
  }
}

/// Parse an `@`-prefixed FormData shorthand value.
///
/// `@path` is a file path. `@filename#path` uses only the first `#` as the
/// filename/path delimiter so later `#` characters stay in the path. The path
/// portion is trimmed; the filename is not.
fn form_data_from_file_shorthand<S: AsRef<str>>(name: S, value: &str) -> FormData {
  let rest = &value[1..];
  match rest.split_once("#") {
    None => FormData::with_file(name, Path::new(rest)),
    Some((filename, path)) => FormData::with_file_and_name(name, Path::new(path.trim()), filename),
  }
}

impl ToFormData for &str {
  /// Support format text
  /// ## sample
  /// ```text
  /// name=Nick&file=@/path/to/file&file_and_filename=@filename#/path/to/file
  /// ```
  fn to_formdatas(&self) -> Vec<FormData> {
    self
      .split("&")
      .collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let (name, value) = part.split_once("=").map_or_else(
          || (part.trim().to_string(), "".to_string()),
          |(name, value)| (name.trim().to_string(), value.trim().to_string()),
        );
        if !value.starts_with("@") {
          return FormData::with_text(name, value);
        }
        form_data_from_file_shorthand(name, &value)
      })
      .filter(|para: &FormData| !para.name.is_empty())
      .collect::<Vec<FormData>>()
  }
}

impl ToFormData for String {
  fn to_formdatas(&self) -> Vec<FormData> {
    (&self[..]).to_formdatas()
  }
}

impl<K: AsRef<str> + Eq + std::hash::Hash, V: AsRef<str>> ToFormData for HashMap<K, V> {
  fn to_formdatas(&self) -> Vec<FormData> {
    let mut rets = Vec::with_capacity(self.len());
    for name in self.keys() {
      if let Some(value) = self.get(name) {
        let value = value.as_ref();
        if !value.starts_with("@") {
          rets.push(FormData::with_text(name, value));
          continue;
        }
        rets.push(form_data_from_file_shorthand(name, value));
      }
    }
    rets
  }
}

impl<IU: ToFormData> ToFormData for &IU {
  fn to_formdatas(&self) -> Vec<FormData> {
    (*self).to_formdatas()
  }
}

impl<IU: ToFormData> ToFormData for &mut IU {
  fn to_formdatas(&self) -> Vec<FormData> {
    (**self).to_formdatas()
  }
}

macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {
    $sub
  };
}

macro_rules! tuple_to_formdata {
  ( $( $item:ident )+ ) => {
    impl<T: ToFormData> ToFormData for (
      $(replace_expr!(
        ($item)
        T
      ),)+
    )
    {
      fn to_formdatas(&self) -> Vec<FormData> {
        let mut rets = vec![];
        let ($($item,)+) = self;
        let mut _name = "".to_string();
        let mut _position = 0;
        $(
          let paras = $item.to_formdatas();
          if !paras.is_empty() {

            let first = paras.get(0);
            let mut first_value_not_empty = false;
            if let Some(v) = first {
              let first_text = v.text();
              if let Some(t) = first_text {
                if !t.is_empty() {
                  first_value_not_empty = true;
                }
              }
            }

            if paras.len() > 1 ||
              paras.get(0).filter(|&v| v.text().is_some() && first_value_not_empty).is_some()
            {
              rets.extend(paras);
              _position = 0;
            } else {
              if let Some(para_first) = paras.get(0) {
                if _position == 0 {
                  _name = para_first.name().clone();
                  _position = 1;
                } else {
                  let value = para_first.name();
                  if !value.starts_with("@") {
                    rets.push(FormData::with_text(&_name, value));
                  } else {
                    rets.push(form_data_from_file_shorthand(&_name, value));
                  }
                  _position = 0;
                }
              }
            }

          }
        )+
        rets
      }
    }
  };
}

tuple_to_formdata! { a }
tuple_to_formdata! { a b }
tuple_to_formdata! { a b c }
tuple_to_formdata! { a b c d }
tuple_to_formdata! { a b c d e }
tuple_to_formdata! { a b c d e f }
tuple_to_formdata! { a b c d e f g }
tuple_to_formdata! { a b c d e f g h }
tuple_to_formdata! { a b c d e f g h i }
tuple_to_formdata! { a b c d e f g h i j }
tuple_to_formdata! { a b c d e f g h i j k }
tuple_to_formdata! { a b c d e f g h i j k l }
tuple_to_formdata! { a b c d e f g h i j k l m }
tuple_to_formdata! { a b c d e f g h i j k l m n }
tuple_to_formdata! { a b c d e f g h i j k l m n o }
tuple_to_formdata! { a b c d e f g h i j k l m n o p }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u v }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u v w }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u v w x }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u v w x y }
tuple_to_formdata! { a b c d e f g h i j k l m n o p q r s t u v w x y z }

#[cfg(test)]
mod tests {
  use super::{FormDataType, ToFormData};
  use std::collections::HashMap;
  use std::path::PathBuf;

  #[test]
  fn preserves_equals_in_shorthand_values_and_trims_whitespace() {
    let formdata = " token = a=b=c & other = spaced = value ".to_formdatas();

    assert_eq!(formdata.len(), 2);
    assert_eq!(formdata[0].name(), "token");
    assert_eq!(formdata[0].text(), &Some("a=b=c".to_string()));
    assert_eq!(formdata[1].name(), "other");
    assert_eq!(formdata[1].text(), &Some("spaced = value".to_string()));
  }

  #[test]
  fn keeps_file_forms_and_filters_empty_names() {
    let formdata =
      "=ignored&file=@/tmp/input.txt&named=@download.txt#/tmp/input.txt".to_formdatas();

    assert_eq!(formdata.len(), 2);
    assert_eq!(formdata[0].name(), "file");
    assert_eq!(formdata[0].type_(), &FormDataType::FILE);
    assert_eq!(formdata[0].file(), &Some(PathBuf::from("/tmp/input.txt")));
    assert_eq!(formdata[1].name(), "named");
    assert_eq!(formdata[1].filename(), &Some("download.txt".to_string()));
    assert_eq!(formdata[1].file(), &Some(PathBuf::from("/tmp/input.txt")));
  }

  #[test]
  fn reuses_shorthand_parsing_for_string_and_tuples() {
    let value = "token=a=b=c".to_string();
    let string_formdata = value.to_formdatas();
    let tuple_formdata = ("token=a=b=c",).to_formdatas();

    assert_eq!(string_formdata[0].text(), &Some("a=b=c".to_string()));
    assert_eq!(tuple_formdata[0].text(), &Some("a=b=c".to_string()));
  }

  #[test]
  fn keeps_hashmap_plain_file_shorthand_to_one_part() {
    let mut values = HashMap::new();
    values.insert("file", "@/tmp/input.txt");

    let formdata = values.to_formdatas();

    assert_eq!(formdata.len(), 1);
    assert_eq!(formdata[0].type_(), &FormDataType::FILE);
    assert_eq!(formdata[0].file(), &Some(PathBuf::from("/tmp/input.txt")));
    assert_eq!(formdata[0].filename(), &Some("input.txt".to_string()));
  }

  #[test]
  fn keeps_hashmap_text_and_named_file_to_one_part_each() {
    let mut values = HashMap::new();
    values.insert("token", "value");
    values.insert("file", "@download.txt#/tmp/input.txt");

    let formdata = values.to_formdatas();

    assert_eq!(formdata.len(), 2);
    let text = formdata.iter().find(|part| part.name() == "token").unwrap();
    assert_eq!(text.text(), &Some("value".to_string()));
    let file = formdata.iter().find(|part| part.name() == "file").unwrap();
    assert_eq!(file.type_(), &FormDataType::FILE);
    assert_eq!(file.file(), &Some(PathBuf::from("/tmp/input.txt")));
    assert_eq!(file.filename(), &Some("download.txt".to_string()));
  }

  #[test]
  fn preserves_hashes_in_named_file_paths_for_string_hashmap_and_tuple() {
    let path = "/tmp/dir#with#hash/input.txt";
    let shorthand = "@download.txt#/tmp/dir#with#hash/input.txt";

    let string_formdata = format!("file={}", shorthand).to_formdatas();
    assert_eq!(string_formdata.len(), 1);
    assert_eq!(string_formdata[0].name(), "file");
    assert_eq!(string_formdata[0].type_(), &FormDataType::FILE);
    assert_eq!(
      string_formdata[0].filename(),
      &Some("download.txt".to_string())
    );
    assert_eq!(string_formdata[0].file(), &Some(PathBuf::from(path)));

    let mut values = HashMap::new();
    values.insert("file", shorthand);
    let hashmap_formdata = values.to_formdatas();
    assert_eq!(hashmap_formdata.len(), 1);
    assert_eq!(hashmap_formdata[0].type_(), &FormDataType::FILE);
    assert_eq!(
      hashmap_formdata[0].filename(),
      &Some("download.txt".to_string())
    );
    assert_eq!(hashmap_formdata[0].file(), &Some(PathBuf::from(path)));

    let tuple_formdata = ("file", shorthand).to_formdatas();
    assert_eq!(tuple_formdata.len(), 1);
    assert_eq!(tuple_formdata[0].name(), "file");
    assert_eq!(tuple_formdata[0].type_(), &FormDataType::FILE);
    assert_eq!(
      tuple_formdata[0].filename(),
      &Some("download.txt".to_string())
    );
    assert_eq!(tuple_formdata[0].file(), &Some(PathBuf::from(path)));
  }

  #[test]
  fn trims_named_file_path_whitespace_and_keeps_plain_file_filename() {
    let named = " named = @download.txt# /tmp/input.txt ".to_formdatas();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].name(), "named");
    assert_eq!(named[0].type_(), &FormDataType::FILE);
    assert_eq!(named[0].filename(), &Some("download.txt".to_string()));
    assert_eq!(named[0].file(), &Some(PathBuf::from("/tmp/input.txt")));

    let plain = "file=@/tmp/input.txt".to_formdatas();
    assert_eq!(plain.len(), 1);
    assert_eq!(plain[0].type_(), &FormDataType::FILE);
    assert_eq!(plain[0].file(), &Some(PathBuf::from("/tmp/input.txt")));
    assert_eq!(plain[0].filename(), &Some("input.txt".to_string()));

    let text = "token=plain".to_formdatas();
    assert_eq!(text[0].text(), &Some("plain".to_string()));
    assert_eq!(text[0].file(), &None);
  }
}
