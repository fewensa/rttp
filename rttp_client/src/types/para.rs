use crate::error;
use crate::error::Error;

#[derive(Clone, Debug)]
pub struct Para {
  name: String,
  value: String,
  array: bool,
}

pub trait IntoPara {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn into_para(self) -> Vec<Para>;
}

impl Para {
  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().into(),
      value: value.as_ref().into(),
      array: false,
    }
  }

  pub fn name(&self) -> &String {
    &self.name
  }

  pub fn value(&self) -> &String {
    &self.value
  }

  pub fn array(&self) -> bool {
    self.array
  }
}

impl IntoPara for Para {
  fn into_para(self) -> Vec<Para> {
    vec![self]
  }
}

impl<'a> IntoPara for &'a str {
  fn into_para(self) -> Vec<Para> {
    self.split("&").collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let pvs: Vec<&str> = part.split("=").collect::<Vec<&str>>();
        Para::new(
          pvs.get(0).map_or("".to_string(), |v| v.to_string()),
          pvs.get(1).map_or("".to_string(), |v| v.to_string()),
        )
      })
      .filter(|para: &Para| !para.name.is_empty())
      .collect::<Vec<Para>>()
  }
}

impl<'a> IntoPara for &'a String {
  fn into_para(self) -> Vec<Para> {
    (&**self).into_para()
  }
}

