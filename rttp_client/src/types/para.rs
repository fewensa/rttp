use std::path::PathBuf;

use crate::error;
use crate::error::Error;
use crate::types::ParaType::FORM;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ParaType {
  FORM,
  FILE,
}

#[derive(Clone, Debug)]
pub struct Para {
  name: String,
  text: Option<String>,
  file: Option<PathBuf>,
  type_: ParaType,
}

pub trait IntoPara {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn into_paras(&self) -> Vec<Para>;
}

impl Para {
  pub fn with_form<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().into(),
      text: Some(value.as_ref().into()),
      file: None,
      type_: ParaType::FORM
    }
  }

  pub fn with_file<N: AsRef<str>, V: AsRef<PathBuf>>(name: N, path: V) -> Self {
    Self {
      name: name.as_ref().into(),
      text: None,
      file: Some(path.as_ref().into()),
      type_: ParaType::FILE
    }
  }

  pub fn name(&self) -> &String {
    &self.name
  }

  pub fn type_(&self) -> &ParaType {
    &self.type_
  }

  pub fn text(&self) -> &Option<String> {
    &self.text
  }

  pub fn file(&self) -> &Option<PathBuf> {
    &self.file
  }

  pub fn is_form(&self) -> bool {
    self.type_ == ParaType::FORM
  }

  pub fn is_file(&self) -> bool {
    self.type_ == ParaType::FILE
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
        Para::with_form(
          pvs.get(0).map_or("".to_string(), |v| v.to_string()).trim(),
          pvs.get(1).map_or("".to_string(), |v| v.to_string()).trim(),
        )
      })
      .filter(|para: &Para| !para.name.is_empty())
      .collect::<Vec<Para>>()
  }
}

impl<'a> IntoPara for &'a String {
  fn into_paras(&self) -> Vec<Para> {
    (&self[..]).into_paras()
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
              position = 0;
            } else {
              if let Some(para_first) = paras.get(0) {
                if position == 0 {
                  name = para_first.name().clone();
                  position = 1;
                } else {
                  rets.push(Para::with_form(&name, para_first.name()));
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


