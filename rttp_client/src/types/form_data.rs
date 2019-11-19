use std::path::{Path, PathBuf};

use crate::types::Para;

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
  fn to_formdatas(&self) -> Vec<FormData> {
    unimplemented!()
  }
}

impl<'a> ToFormData for &'a String {
  fn to_formdatas(&self) -> Vec<FormData> {
    (&self[..]).to_formdatas()
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

