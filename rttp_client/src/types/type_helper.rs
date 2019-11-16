


pub fn safe_uri(uri: String) -> String {
  uri.split("/")
    .collect::<Vec<&str>>()
    .iter()
    .filter(|v| !v.is_empty())
    .map(|v| v.to_string())
    .collect::<Vec<String>>()
    .join("/")
}

