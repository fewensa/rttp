use crate::request::builder::common::RawBuilder;
use crate::types::RoUrl;

// rebuild para/url
impl<'a> RawBuilder<'a> {
  pub fn rebuild_paras(&mut self, rourl: &mut RoUrl) {
    let traditional = self.request.traditional();
    rourl.traditional(traditional);

    let mut formdata_req = self.request.formdatas().clone();
    let mut paras_req = self.request.paras().clone();
    let mut paras_url = rourl.paras_get().clone();

    let mut para_is_array: Vec<(String, bool)> =
      Vec::with_capacity(paras_req.len() + paras_url.len());

    formdata_req.iter().for_each(|p| {
      if let Some(v) = para_is_array.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        para_is_array.push((p.name().clone(), false));
      }
    });
    paras_req.iter().for_each(|p| {
      if let Some(v) = para_is_array.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        para_is_array.push((p.name().clone(), false));
      }
    });
    paras_url.iter().for_each(|p| {
      if let Some(v) = para_is_array.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        para_is_array.push((p.name().clone(), false));
      }
    });

    formdata_req.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = para_is_array.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    paras_req.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = para_is_array.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    paras_url.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = para_is_array.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    self.request.formdatas_set(formdata_req);
    self.request.paras_set(paras_req);
    rourl.paras(paras_url);
  }

  pub fn rebuild_url(&mut self, rourl: &mut RoUrl) {
    self.request.paths().iter().for_each(|path| {
      rourl.path(path);
    });
  }
}
