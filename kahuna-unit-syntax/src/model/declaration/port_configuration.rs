use proc_macro2::Ident;
use syn::Type;
use kahuna_core::{Rate, StreamKind};
use crate::{DefaultValue};

pub struct PortConfiguration {
  pub direction: Direction,
  pub identifier: Ident,
  pub stream_kind: StreamKind,
  pub rate_by_type: Option<Rate>,
  pub rate_override: Option<Rate>,
  pub default_value: Option<DefaultValue>,
  pub value_type: Type,
}

impl PortConfiguration {
  pub fn rate(&self) -> Option<Rate> {
    self.rate_override.or(self.rate_by_type)
  }

  /// `None` for outputs, where the concept doesn't apply.
  pub fn required(&self) -> Option<bool> {
    match (self.direction, self.stream_kind) {
      (Direction::Output, _) => None,
      (Direction::Input, StreamKind::Parameter) => Some(self.default_value.is_none()),
      (Direction::Input, StreamKind::Signal) => Some(true),
      (Direction::Input, StreamKind::Event) => Some(false),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Input,
  Output,
}
