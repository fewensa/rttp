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

  pub fn replace(&mut self, header: Header) -> &mut Self {
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
          .join("");
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
        let mut name = "".to_string();
        let mut position = 0;
        $(
          let headers = $item.into_headers();
          if !headers.is_empty() {

            if headers.len() > 1 ||
              headers.get(0).filter(|&v| !v.value().is_empty()).is_some()
            {
              rets.extend(headers);
              position = 0;
            } else {
              if let Some(first) = headers.get(0) {
                if position == 0 {
                  name = first.name().clone();
                  position = 1;
                } else {
                  rets.push(Header::new(&name, first.name()));
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

tuple_to_header! { A }
tuple_to_header! { A B }
tuple_to_header! { A B C }
tuple_to_header! { A B C D }
tuple_to_header! { A B C D E }
tuple_to_header! { A B C D E F }
tuple_to_header! { A B C D E F G }
tuple_to_header! { A B C D E F G H }
tuple_to_header! { A B C D E F G H I }
tuple_to_header! { A B C D E F G H I J }
tuple_to_header! { A B C D E F G H I J K }
tuple_to_header! { A B C D E F G H I J K L }
tuple_to_header! { A B C D E F G H I J K L M }
tuple_to_header! { A B C D E F G H I J K L M N }
tuple_to_header! { A B C D E F G H I J K L M N O }
tuple_to_header! { A B C D E F G H I J K L M N O P }
tuple_to_header! { A B C D E F G H I J K L M N O P Q }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U V }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U V W }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U V W X }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
tuple_to_header! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }



