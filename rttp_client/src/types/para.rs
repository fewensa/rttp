use std::collections::HashMap;
use std::path::PathBuf;

use crate::error;
use crate::error::Error;
use crate::types::FormData;
use crate::types::ParaType::FORM;

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
  array: bool
}

pub trait IntoPara {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn into_paras(&self) -> Vec<Para>;
}

impl Para {
  pub(crate) fn with_url<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().into(),
      value: Some(value.as_ref().into()),
      type_: ParaType::URL,
      array: false
    }
  }

  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().into(),
      value: Some(value.as_ref().into()),
      type_: ParaType::FORM,
      array: false
    }
  }

  pub fn name(&self) -> &String { &self.name }
  pub fn type_(&self) -> &ParaType { &self.type_ }
  pub fn value(&self) -> &Option<String> { &self.value }
  pub fn array(&self) -> bool { self.array }

  pub fn is_url(&self) -> bool { self.type_ == ParaType::URL }
  pub fn is_form(&self) -> bool { self.type_ == ParaType::FORM }

  pub(crate) fn name_mut(&mut self) -> &mut String { &mut self.name }
  pub(crate) fn type_mut(&mut self) -> &mut ParaType { &mut self.type_ }
  pub(crate) fn value_mut(&mut self) -> &mut Option<String> { &mut self.value }
  pub(crate) fn array_mut(&mut self) -> &mut bool { &mut self.array }
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
    self.split("&").collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let pvs: Vec<&str> = part.split("=").collect::<Vec<&str>>();
        Para::new(
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
        rets.push(Para::new(key, value))
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

//impl IntoPara for (&str, &str) {
//  fn into_paras(self) -> Vec<Para> {
//    let para = Para::with_form(self.0, self.1);
//    vec![para]
//  }
//}

//impl<T: AsRef<str>> IntoPara for (T, T) {
//  fn into_paras(self) -> Vec<Para> {
//    let para = Para::with_form(self.0.as_ref(), self.1.as_ref());
//    vec![para]
//  }
//}

macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {$sub};
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
        let mut name = "".to_string();
        let mut position = 0;
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
              position = 0;
            } else {
              if let Some(para_first) = paras.get(0) {
                if position == 0 {
                  name = para_first.name().clone();
                  position = 1;
                } else {
                  rets.push(Para::new(&name, para_first.name()));
                  position = 0;
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

tuple_to_para! { A }
tuple_to_para! { A B }
tuple_to_para! { A B C }
tuple_to_para! { A B C D }
tuple_to_para! { A B C D E }
tuple_to_para! { A B C D E F }
tuple_to_para! { A B C D E F G }
tuple_to_para! { A B C D E F G H }
tuple_to_para! { A B C D E F G H I }
tuple_to_para! { A B C D E F G H I J }
tuple_to_para! { A B C D E F G H I J K }
tuple_to_para! { A B C D E F G H I J K L }
tuple_to_para! { A B C D E F G H I J K L M }
tuple_to_para! { A B C D E F G H I J K L M N }
tuple_to_para! { A B C D E F G H I J K L M N O }
tuple_to_para! { A B C D E F G H I J K L M N O P }
tuple_to_para! { A B C D E F G H I J K L M N O P Q }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U V }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U V W }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U V W X }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
tuple_to_para! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }


