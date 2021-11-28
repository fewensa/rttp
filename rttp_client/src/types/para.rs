use std::collections::HashMap;

use crate::types::FormData;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ParaType {
  URL,
  FORM,
}

#[derive(Clone, Debug)]
pub struct Para {
  name: String,
  value: Option<String>,
  type_: ParaType,
  array: bool,
}

pub trait IntoPara {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn into_paras(&self) -> Vec<Para>;
}

impl Para {
  pub fn with_url(name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
    Self {
      name: name.as_ref().into(),
      value: Some(value.as_ref().into()),
      type_: ParaType::URL,
      array: false,
    }
  }

  pub fn with_form(name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
    Self {
      name: name.as_ref().into(),
      value: Some(value.as_ref().into()),
      type_: ParaType::FORM,
      array: false,
    }
  }

  pub fn name(&self) -> &String {
    &self.name
  }
  pub fn type_(&self) -> &ParaType {
    &self.type_
  }
  pub fn value(&self) -> &Option<String> {
    &self.value
  }
  pub fn array(&self) -> bool {
    self.array
  }

  pub fn is_url(&self) -> bool {
    self.type_ == ParaType::URL
  }
  pub fn is_form(&self) -> bool {
    self.type_ == ParaType::FORM
  }

  #[allow(dead_code)]
  pub(crate) fn name_mut(&mut self) -> &mut String {
    &mut self.name
  }
  pub(crate) fn type_mut(&mut self) -> &mut ParaType {
    &mut self.type_
  }
  #[allow(dead_code)]
  pub(crate) fn value_mut(&mut self) -> &mut Option<String> {
    &mut self.value
  }
  pub(crate) fn array_mut(&mut self) -> &mut bool {
    &mut self.array
  }
}

impl Para {
  pub fn to_formdata(&self) -> FormData {
    if let Some(value) = self.value() {
      FormData::with_text(self.name(), value)
    } else {
      FormData::with_text(self.name(), "")
    }
  }
}

impl IntoPara for Para {
  fn into_paras(&self) -> Vec<Para> {
    vec![self.clone()]
  }
}

impl<'a> IntoPara for &'a str {
  fn into_paras(&self) -> Vec<Para> {
    self
      .split("&")
      .collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let pvs: Vec<&str> = part.split("=").collect::<Vec<&str>>();
        Para::with_form(
          pvs.get(0).map_or("".to_string(), |v| v.to_string()).trim(),
          pvs.get(1).map_or("".to_string(), |v| v.to_string()).trim(),
        )
      })
      .filter(|para: &Para| !para.name.is_empty())
      .collect::<Vec<Para>>()
  }
}

impl IntoPara for String {
  fn into_paras(&self) -> Vec<Para> {
    (&self[..]).into_paras()
  }
}

impl<K: AsRef<str> + Eq + std::hash::Hash, V: AsRef<str>> IntoPara for HashMap<K, V> {
  fn into_paras(&self) -> Vec<Para> {
    let mut rets = Vec::with_capacity(self.len());
    for key in self.keys() {
      if let Some(value) = self.get(key) {
        rets.push(Para::with_form(key, value))
      }
    }
    rets
  }
}

impl<'a, IU: IntoPara> IntoPara for &'a IU {
  fn into_paras(&self) -> Vec<Para> {
    (*self).into_paras()
  }
}

impl<'a, IU: IntoPara> IntoPara for &'a mut IU {
  fn into_paras(&self) -> Vec<Para> {
    (**self).into_paras()
  }
}

macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {
    $sub
  };
}

macro_rules! tuple_to_para {
  ( $( $item:ident )+ ) => {
    impl<T: IntoPara> IntoPara for (
      $(replace_expr!(
        ($item)
        T
      ),)+
    )
    {
      fn into_paras(&self) -> Vec<Para> {
        let mut rets = vec![];
        let ($($item,)+) = self;
        let mut _name = "".to_string();
        let mut _position = 0;
        $(
          let paras = $item.into_paras();
          if !paras.is_empty() {

            // check first para have text(value), if true, is an independent para
            let first = paras.get(0);
            let mut first_value_not_empty = false;
            if let Some(v) = first {
              let first_text = v.value();
              if let Some(t) = first_text {
                if !t.is_empty() {
                  first_value_not_empty = true;
                }
              }
            }

            if paras.len() > 1 ||
              paras.get(0).filter(|&v| v.value().is_some() && first_value_not_empty).is_some()
            {
              rets.extend(paras);
              _position = 0;
            } else {
              if let Some(para_first) = paras.get(0) {
                if _position == 0 {
                  _name = para_first.name().clone();
                  _position = 1;
                } else {
                  rets.push(Para::with_form(&_name, para_first.name()));
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

tuple_to_para! { a }
tuple_to_para! { a b }
tuple_to_para! { a b c }
tuple_to_para! { a b c d }
tuple_to_para! { a b c d e }
tuple_to_para! { a b c d e f }
tuple_to_para! { a b c d e f g }
tuple_to_para! { a b c d e f g h }
tuple_to_para! { a b c d e f g h i }
tuple_to_para! { a b c d e f g h i j }
tuple_to_para! { a b c d e f g h i j k }
tuple_to_para! { a b c d e f g h i j k l }
tuple_to_para! { a b c d e f g h i j k l m }
tuple_to_para! { a b c d e f g h i j k l m n }
tuple_to_para! { a b c d e f g h i j k l m n o }
tuple_to_para! { a b c d e f g h i j k l m n o p }
tuple_to_para! { a b c d e f g h i j k l m n o p q }
tuple_to_para! { a b c d e f g h i j k l m n o p q r }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u v }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u v w }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u v w x }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u v w x y }
tuple_to_para! { a b c d e f g h i j k l m n o p q r s t u v w x y z }
