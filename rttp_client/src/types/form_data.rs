use std::path::{Path, PathBuf};
use std::collections::HashMap;

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
    let filename = file.file_name()
      .map_or("".to_string(), |v| v.to_string_lossy().to_string());
    Self::with_file_and_name(name, file, filename)
  }

  pub fn with_file_and_name<S: AsRef<str>, N: AsRef<str>, P: AsRef<Path>>(name: S, file: P, filename: N) -> Self {
    let filename = filename.as_ref();
    let filename = if filename.is_empty() { None } else { Some(filename.to_string()) };
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

  pub fn name(&self) -> &String { &self.name }
  pub fn text(&self) -> &Option<String> { &self.text }
  pub fn file(&self) -> &Option<PathBuf> { &self.file }
  pub fn filename(&self) -> &Option<String> { &self.filename }
  pub fn binary(&self) -> &Vec<u8> { &self.binary }
  pub fn type_(&self) -> &FormDataType { &self.type_ }
  pub fn array(&self) -> bool { self.array }

  pub fn is_text(&self) -> bool { self.type_ == FormDataType::TEXT }
  pub fn is_file(&self) -> bool { self.type_ == FormDataType::FILE }
  pub fn is_binary(&self) -> bool { self.type_ == FormDataType::BINARY }

  pub(crate) fn name_mut(&mut self) -> &mut String { &mut self.name }
  pub(crate) fn text_mut(&mut self) -> &mut Option<String> { &mut self.text }
  pub(crate) fn file_mut(&mut self) -> &mut Option<PathBuf> { &mut self.file }
  pub(crate) fn filename_mut(&mut self) -> &mut Option<String> { &mut self.filename }
  pub(crate) fn binary_mut(&mut self) -> &mut Vec<u8> { &mut self.binary }
  pub(crate) fn type_mut(&mut self) -> &mut FormDataType { &mut self.type_ }
  pub(crate) fn array_mut(&mut self) -> &mut bool { &mut self.array }
}

impl ToFormData for FormData {
  fn to_formdatas(&self) -> Vec<FormData> {
    vec![self.clone()]
  }
}

impl<'a> ToFormData for &'a str {
  /// Support format text
  /// ## sample
  /// ```text
  /// name=Nick&file=@/path/to/file&file_and_filename=@filename#/path/to/file
  /// ```
  fn to_formdatas(&self) -> Vec<FormData> {
    self.split("&").collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let pvs: Vec<&str> = part.split("=").collect::<Vec<&str>>();
        let name = pvs.get(0).map_or("".to_string(), |v| v.trim().to_string());
        let value = pvs.get(1).map_or("".to_string(), |v| v.trim().to_string());
        if !value.starts_with("@") {
          return FormData::with_text(name, value);
        }
        if !value.contains("#") {
          let path = Path::new(&value[1..]);
          return FormData::with_file(name, path);
        }
        let hasps: Vec<&str> = (&value[1..]).split("#").collect::<Vec<&str>>();
        let len = hasps.len();
        let filename = hasps.iter().enumerate().filter(|(ix, _)| ix + 1 < len)
          .map(|(_, &v)| v)
          .collect::<Vec<&str>>()
          .join("#");
        let path = hasps.get(len - 1).map_or("".to_string(), |v| v.trim().to_string());
        let path = Path::new(&path);
        FormData::with_file_and_name(name, path, filename)
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
          rets.push(FormData::with_text(&name, value));
          continue;
        }
        if !value.contains("#") {
          let path = Path::new(&value[1..]);
          rets.push(FormData::with_file(&name, path));
        }
        let hasps: Vec<&str> = (&value[1..]).split("#").collect::<Vec<&str>>();
        let len = hasps.len();
        let filename = hasps.iter().enumerate().filter(|(ix, _)| ix + 1 < len)
          .map(|(_, &v)| v)
          .collect::<Vec<&str>>()
          .join("#");
        let path = hasps.get(len - 1).map_or("".to_string(), |v| v.trim().to_string());
        let path = Path::new(&path);
        rets.push(FormData::with_file_and_name(&name, path, filename));
      }
    }
    rets
  }
}


impl<'a, IU: ToFormData> ToFormData for &'a IU {
  fn to_formdatas(&self) -> Vec<FormData> {
    (*self).to_formdatas()
  }
}

impl<'a, IU: ToFormData> ToFormData for &'a mut IU {
  fn to_formdatas(&self) -> Vec<FormData> {
    (**self).to_formdatas()
  }
}




macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {$sub};
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
        let mut name = "".to_string();
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
                  name = para_first.name().clone();
                  _position = 1;
                } else {
                  let value = para_first.name();
                  if !value.starts_with("@") {
                    rets.push(FormData::with_text(&name, value));
                  } else {
                    if !value.contains("#") {
                      let path = Path::new(&value[1..]);
                      rets.push(FormData::with_file(&name, path));
                    } else {
                      let hasps: Vec<&str> = (&value[1..]).split("#").collect::<Vec<&str>>();
                      let len = hasps.len();
                      let filename = hasps.iter().enumerate().filter(|(ix, _)| ix + 1 < len)
                        .map(|(_, &v)| v)
                        .collect::<Vec<&str>>()
                        .join("#");
                      let path = hasps.get(len - 1).map_or("".to_string(), |v| v.trim().to_string());
                      let path = Path::new(&path);
                      rets.push(FormData::with_file_and_name(&name, path, filename));
                    }
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

