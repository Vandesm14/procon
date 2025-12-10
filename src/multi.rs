use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(untagged)]
pub enum Multi<T> {
  #[default]
  None,
  Single(T),
  Many(Vec<T>),
}

impl<T> Multi<T> {
  pub fn to_vec(&self) -> Vec<T>
  where
    T: Clone,
  {
    match self {
      Multi::None => Vec::new(),
      Multi::Single(t) => vec![t.clone()],
      Multi::Many(ts) => ts.clone(),
    }
  }

  pub fn to_option(&self) -> Option<Vec<T>>
  where
    T: Clone,
  {
    match self {
      Multi::None => None,
      Multi::Single(t) => Some(vec![t.clone()]),
      Multi::Many(ts) => Some(ts.clone()),
    }
  }
}
