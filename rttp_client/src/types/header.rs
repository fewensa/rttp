use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Header {
  name: String,
  value: String,
}


pub trait IntoHeader {
  fn into_headers(&self) -> Vec<Header>;
}

impl Header {
  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().trim().into(),
      value: value.as_ref().trim().into(),
    }
  }

  pub(crate) fn replace(&mut self, header: Header) -> &mut Self {
    self.name = header.name().clone();
    self.value = header.value().clone();
    self
  }

  pub fn name(&self) -> &String {
    &self.name
  }

  pub fn value(&self) -> &String {
    &self.value
  }

  pub fn value_as_isize(&self) -> Result<isize, std::num::ParseIntError> {
    self.value.parse()
  }

  pub fn value_as_usize(&self) -> Result<usize, std::num::ParseIntError> {
    self.value.parse()
  }
}

impl<'a> IntoHeader for &'a str {
  fn into_headers(&self) -> Vec<Header> {
    self.split("\n").collect::<Vec<&str>>()
      .iter()
      .map(|part: &&str| {
        let pvs: Vec<&str> = part.split(":").collect::<Vec<&str>>();
        let name = pvs.get(0);
        let value = pvs.iter().enumerate()
          .filter(|(ix, _)| *ix > 0)
          .map(|(_, v)| v.to_string())
          .collect::<Vec<String>>()
          .join(":");
        Header::new(
          name.map_or("".to_string(), |v| v.to_string()).trim(),
          value.trim(),
        )
      })
      .filter(|header: &Header| !header.name.is_empty())
      .collect::<Vec<Header>>()
  }
}


impl IntoHeader for String {
  fn into_headers(&self) -> Vec<Header> {
    (&self[..]).into_headers()
  }
}


impl IntoHeader for Header {
  fn into_headers(&self) -> Vec<Header> {
    vec![self.clone()]
  }
}

impl<K: AsRef<str> + Eq + std::hash::Hash, V: AsRef<str>> IntoHeader for HashMap<K, V> {
  fn into_headers(&self) -> Vec<Header> {
    let mut rets = Vec::with_capacity(self.len());
    for key in self.keys() {
      if let Some(value) = self.get(key) {
        rets.push(Header::new(key, value))
      }
    }
    rets
  }
}


impl<'a, IU: IntoHeader> IntoHeader for &'a IU {
  fn into_headers(&self) -> Vec<Header> {
    (*self).into_headers()
  }
}

impl<'a, IU: IntoHeader> IntoHeader for &'a mut IU {
  fn into_headers(&self) -> Vec<Header> {
    (**self).into_headers()
  }
}






macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {$sub};
}

macro_rules! tuple_to_header {
  ( $( $item:ident )+ ) => {
    impl<T: IntoHeader> IntoHeader for (
      $(replace_expr!(
        ($item)
        T
      ),)+
    )
    {
      fn into_headers(&self) -> Vec<Header> {
        let mut rets = vec![];
        let ($($item,)+) = self;
        let mut _name = "".to_string();
        let mut _position = 0;
        $(
          let headers = $item.into_headers();
          if !headers.is_empty() {

            if headers.len() > 1 ||
              headers.get(0).filter(|&v| !v.value().is_empty()).is_some()
            {
              rets.extend(headers);
              _position = 0;
            } else {
              if let Some(first) = headers.get(0) {
                if _position == 0 {
                  _name = first.name().clone();
                  _position = 1;
                } else {
                  rets.push(Header::new(&_name, first.name()));
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

tuple_to_header! { a }
tuple_to_header! { a b }
tuple_to_header! { a b c }
tuple_to_header! { a b c d }
tuple_to_header! { a b c d e }
tuple_to_header! { a b c d e f }
tuple_to_header! { a b c d e f g }
tuple_to_header! { a b c d e f g h }
tuple_to_header! { a b c d e f g h i }
tuple_to_header! { a b c d e f g h i j }
tuple_to_header! { a b c d e f g h i j k }
tuple_to_header! { a b c d e f g h i j k l }
tuple_to_header! { a b c d e f g h i j k l m }
tuple_to_header! { a b c d e f g h i j k l m n }
tuple_to_header! { a b c d e f g h i j k l m n o }
tuple_to_header! { a b c d e f g h i j k l m n o p }
tuple_to_header! { a b c d e f g h i j k l m n o p q }
tuple_to_header! { a b c d e f g h i j k l m n o p q r }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x y }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x y z }



